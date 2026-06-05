use std::{collections::HashMap, process::Command};

use sqlx::{Pool, Sqlite};

use crate::{
    bootloader::{
        parser::ConfigFileParser,
        systemd_boot::{
            boot_entries::{EntryConfigFile, SystemdBootEntries},
            loader_config::LoaderConfigFile,
        },
    },
    config::SYSTEMD_CFG_PATH,
    data::{
        types::{
            BootkitBootEntries, BootkitBootEntry, BootkitConfig, BootkitSnapshot,
            BootkitSnapshotConfig, BootkitSnapshotSelect, BootkitSnapshots,
        },
        BootkitDataHandler,
    },
    db::systemd_boot::{SystemdBootSnapshot, SystemdDb},
    dctx,
    errors::{DError, DRes, DResult},
};

fn sytemd_snapshot_into_bootkit_snapshot(
    snapshot: SystemdBootSnapshot,
    comparison: &SystemdBootSnapshot,
) -> BootkitSnapshot {
    let configs = HashMap::from([
        (
            SYSTEMD_CFG_PATH.to_string(),
            BootkitSnapshotConfig::with_diff(snapshot.loader_config, &comparison.loader_config),
        ),
        (
            snapshot.selected_entry.clone(),
            BootkitSnapshotConfig::with_diff(snapshot.entry_config, &comparison.entry_config),
        ),
    ]);

    BootkitSnapshot {
        configs,
        id: snapshot.id,
        created: snapshot.created,
        // TODO: read boot entry info to get more kernel data
        kernel: Some(BootkitBootEntry {
            name: snapshot.selected_entry,
            title: None,
        }),
    }
}
/// Helpers for the bootctl commands
struct Bootctl {}

impl Bootctl {
    /// Helper for `bootctl set-default <id>
    ///
    /// Note that the command will accept ANY id as valid.
    /// Make sure the id is actually valid before calling this.
    fn set_default(id: &str) -> DResult<()> {
        let bootctl = Command::new("bootctl")
            .arg("set-default")
            .arg(id)
            .output()
            .ctx(dctx!(), "Failed to run bootctl set-default")?;

        log::trace!("bootctl set-default {id} status: '{:?}'", bootctl.status);
        log::trace!(
            "bootctl set-default {id} stdout: '{}'",
            String::from_utf8_lossy(&bootctl.stdout)
        );
        log::trace!(
            "bootctl set-default {id} stderr: '{}'",
            String::from_utf8_lossy(&bootctl.stderr)
        );

        if !bootctl.status.success() {
            Err(DError::generic(
                dctx!(),
                format!("bootctl set-default {id} failed with {:?}", bootctl.status),
            ))
        } else {
            Ok(())
        }
    }
}

#[derive(Clone)]
pub struct SystemdDataHandler {
    db: SystemdDb,
}

impl SystemdDataHandler {
    pub fn new(pool: Pool<Sqlite>) -> Self {
        Self {
            db: SystemdDb::new(pool),
        }
    }

    fn use_snapshot(&self, snapshot: SystemdBootSnapshot) -> DResult<()> {
        // TODO: check if entry doesn't exist anymore
        let snapshot_entry = EntryConfigFile::new(snapshot.selected_entry, &snapshot.entry_config)
            .ctx(dctx!(), "Failed to get bootentry config from snapshot")?;
        log::trace!("Snapshot boot entry: {snapshot_entry:#?}");

        Bootctl::set_default(snapshot_entry.id())
            .ctx(dctx!(), "Failed to set default entry with bootctl")?;

        snapshot_entry
            .save()
            .ctx(dctx!(), "Failed to save kernel entry config")?;

        let snapshot_config = LoaderConfigFile::new(SYSTEMD_CFG_PATH, &snapshot.loader_config)
            .ctx(dctx!(), "Failed to parse systemd config")?;
        snapshot_config
            .save()
            .ctx(dctx!(), "Failed to save systemd-boot loader config")?;

        Ok(())
    }
}

impl BootkitDataHandler for SystemdDataHandler {
    async fn get_config(&self) -> DResult<BootkitConfig> {
        let snapshot = self
            .db
            .current_snapshot()
            .await
            .ctx(dctx!(), "Failed to fetch current snapshot")?;

        let loader_conf = LoaderConfigFile::new(SYSTEMD_CFG_PATH, &snapshot.loader_config)
            .ctx(dctx!(), "Failed to parse snapshot loader config")?;
        let entry_config = EntryConfigFile::new(&snapshot.selected_entry, &snapshot.entry_config)
            .ctx(dctx!(), "Failed to parse snapshot entry config")?;

        let timeout = loader_conf
            .get_key_value("timeout")
            .map(|kv| kv.value.clone());

        let bootentries =
            SystemdBootEntries::new().ctx(dctx!(), "Failed to get systemd-boot bootentries")?;

        // TODO: difference between system's selected entry and snapshot entry
        //       should be reported to user as it's not expected behavior
        let kernel_arguments = entry_config.options().map(str::to_string);
        let selected_entry = snapshot.selected_entry;

        let entries = bootentries
            .entries
            .iter()
            .map(|entry| BootkitBootEntry {
                name: entry.id().into(),
                title: entry.title().map(str::to_string),
            })
            .collect();

        Ok(BootkitConfig {
            timeout,
            kernel_arguments,
            boot_entries: BootkitBootEntries {
                selected: Some(selected_entry),
                boot_entries: entries,
            },
        })
    }

    async fn get_snapshots(&self) -> DResult<BootkitSnapshots> {
        let snapshots = self
            .db
            .snapshots()
            .await
            .ctx(dctx!(), "Failed to fetch snapshots")?;
        let selected_id = self
            .db
            .selected_snapshot_id()
            .await
            .ctx(dctx!(), "Failed to fetch selected snapshot id")?;
        let current = self
            .db
            .current_snapshot()
            .await
            .ctx(dctx!(), "Failed to fetch current snapshot")?;

        let snapshots = snapshots
            .into_iter()
            .map(|snapshot| sytemd_snapshot_into_bootkit_snapshot(snapshot, &current))
            .collect();

        Ok(BootkitSnapshots {
            snapshots,
            selected: selected_id,
        })
    }

    async fn save_config(&self, config: &BootkitConfig) -> DResult<()> {
        log::debug!("Start saving sytemd-boot config snapshot");
        log::trace!("Config: {config:?}");
        // TODO: if any of this fails, revert to original configs
        // TODO: check if the config has actual changes to avoid duplicate snapshots

        let mut selected_entry = SystemdBootEntries::selected_config_file(config)
            .ctx(dctx!(), "Failed to get selected bootentry config")?;
        log::trace!("Selected boot entry: {selected_entry:#?}");

        Bootctl::set_default(selected_entry.id())
            .ctx(dctx!(), "Failed to set default entry with bootctl")?;

        selected_entry.update_config(config);
        selected_entry
            .save()
            .ctx(dctx!(), "Failed to save kernel entry config")?;

        // TODO: edit the selected/current snapshot instead?
        //       that we we avoid weird situations when 3rd party is manally editing a file
        let mut loader_config = LoaderConfigFile::from_file(SYSTEMD_CFG_PATH)
            .ctx(dctx!(), "Failed to parse systemd config")?;

        loader_config.update_config(config);
        loader_config
            .save()
            .ctx(dctx!(), "Failed to save systemd-boot loader config")?;

        self.db
            .save_systemd_boot(&loader_config, &selected_entry)
            .await
            .ctx(dctx!(), "Failed to save systemd-boot snapshot")?;

        self.db
            .set_selected_snapshot(None)
            .await
            .ctx(dctx!(), "Failed to reset selected snapshot id")?;

        log::debug!("Successfully saved sytemd-boot config snapshot");
        Ok(())
    }

    async fn select_snapshot(&self, select: &BootkitSnapshotSelect) -> DResult<()> {
        log::debug!("Start selecting sytemd-boot snapshot");
        let snapshot = self
            .db
            .snapshot(select.snapshot_id)
            .await
            .ctx(dctx!(), "Failed to get systemd-boot snapshot")?;
        log::trace!("Snapshot: {snapshot:?}");

        self.use_snapshot(snapshot)
            .ctx(dctx!(), "Failed to use selected snapshot")?;

        self.db
            .set_selected_snapshot(Some(select.snapshot_id))
            .await
            .ctx(dctx!(), "Failed to set selected snapshot id to db")?;

        log::debug!("Successfully selected sytemd-boot snapshot");

        Ok(())
    }

    async fn use_current_snapshot(&self) -> DResult<()> {
        // TODO: a lot of the logic should be shared with select snapshot
        log::debug!("Start using current systemd-boot snapshot");
        let snapshot = self
            .db
            .current_snapshot()
            .await
            .ctx(dctx!(), "Failed to get current systemd-boot snapshot")?;
        log::trace!("Snapshot: {snapshot:?}");

        self.use_snapshot(snapshot)
            .ctx(dctx!(), "Failed to use current snapshot")?;
        log::debug!("Successfully used current systemd-boot snapshot");
        Ok(())
    }

    async fn remove_snapshot(&self, select: &BootkitSnapshotSelect) -> DResult<()> {
        log::debug!("Start removing sytemd-boot snapshot");
        let current = self
            .db
            .current_snapshot()
            .await
            .ctx(dctx!(), "Failed to fetch current snapshot")?;

        if current.id == select.snapshot_id {
            return Err(DError::generic(
                dctx!(),
                "Cannot remove snapshot that's in use",
            ));
        }

        self.db
            .remove_snapshot(select.snapshot_id)
            .await
            .ctx(dctx!(), "Failed to remove snapshot")?;

        log::debug!("Succesfully removed sytemd-boot snapshot");

        Ok(())
    }
}
