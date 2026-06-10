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
            BootkitBootEntries, BootkitBootEntry, BootkitConfig, BootkitConfigRaw,
            BootkitConfigsRaw, BootkitRawFile, BootkitSnapshot, BootkitSnapshotConfig,
            BootkitSnapshotSelect, BootkitSnapshots,
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
) -> DResult<BootkitSnapshot> {
    let entry_file = EntryConfigFile::new(&snapshot.selected_entry, &snapshot.entry_config).ctx(
        dctx!(),
        "Failed to get entry config file from snapshot data",
    )?;

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

    Ok(BootkitSnapshot {
        configs,
        id: snapshot.id,
        created: snapshot.created,
        kernel: Some(BootkitBootEntry {
            title: entry_file.version().map(str::to_string),
            name: snapshot.selected_entry,
        }),
    })
}

fn entry_config_diffs(
    snapshot: &SystemdBootSnapshot,
    loader_config: &LoaderConfigFile,
    entry_config: &EntryConfigFile,
    system_entries: &SystemdBootEntries,
) -> DResult<Option<HashMap<String, String>>> {
    let system_loader = LoaderConfigFile::from_file(SYSTEMD_CFG_PATH)
        .ctx(dctx!(), "Failed to parse system loader config")?;
    let system_entry = EntryConfigFile::from_id(&snapshot.selected_entry)
        .ctx(dctx!(), "Failed to parse system entry config")?;

    let loader_diff = loader_config
        .compare_diff(&system_loader)
        .map(|diff| (SYSTEMD_CFG_PATH.to_string(), diff));
    let system_diff = entry_config
        .compare_diff(&system_entry)
        // TODO: get the full file path
        .map(|diff| (entry_config.id().to_string(), diff));

    let kernel_diff = if snapshot.selected_entry != system_entries.selected.id() {
        Some((
            "systemd default entry".to_string(),
            format!(
                "{} -> {}",
                snapshot.selected_entry,
                system_entries.selected.id()
            ),
        ))
    } else {
        None
    };

    let diffs: HashMap<String, String> = [loader_diff, system_diff, kernel_diff]
        .into_iter()
        .flatten()
        .collect();

    if diffs.is_empty() {
        Ok(None)
    } else {
        Ok(Some(diffs))
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

        let config_diffs = entry_config_diffs(&snapshot, &loader_conf, &entry_config, &bootentries)
            .ctx(dctx!(), "Failed to generate config file diffs")?;

        let kernel_arguments = entry_config.options().map(str::to_string);
        let selected_entry = snapshot.selected_entry;

        let entries = bootentries
            .entries
            .iter()
            .map(|entry| BootkitBootEntry {
                name: entry.id().into(),
                title: Some(entry.fancy_title()),
            })
            .collect();

        Ok(BootkitConfig {
            timeout,
            kernel_arguments,
            config_diffs,
            boot_entries: BootkitBootEntries {
                selected: Some(selected_entry),
                boot_entries: entries,
            },
            console: None,
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

        let snapshots: DResult<Vec<BootkitSnapshot>> = snapshots
            .into_iter()
            .map(|snapshot| sytemd_snapshot_into_bootkit_snapshot(snapshot, &current))
            .collect();

        let snapshots = snapshots.ctx(
            dctx!(),
            "Failed to turn systemd-boot snapshot data into snapshots",
        )?;

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

    async fn snapshot_from_system(&self) -> DResult<()> {
        let config = LoaderConfigFile::from_file(SYSTEMD_CFG_PATH)
            .ctx(dctx!(), "Failed to parse systemd config")?;

        let entries = SystemdBootEntries::new()
            .ctx(dctx!(), "Failed to get systemd-boot boot entry information")?;
        let selected = entries.selected.as_file().ctx(
            dctx!(),
            "Expected selected boot entry to not be auto detected entry",
        )?;

        self.db
            .save_systemd_boot(&config, selected)
            .await
            .ctx(dctx!(), "Failed to save systemd-boot snapshot")?;

        self.db
            .set_selected_snapshot(None)
            .await
            .ctx(dctx!(), "Failed to reset selected snapshot id")?;

        log::debug!("Succesfully created a new snapshot from system state");
        Ok(())
    }

    async fn get_configs_raw(&self) -> DResult<BootkitConfigsRaw> {
        let snapshot = self
            .db
            .current_snapshot()
            .await
            .ctx(dctx!(), "Failed to fetch current snapshot")?;

        let loader_conf = LoaderConfigFile::new(SYSTEMD_CFG_PATH, &snapshot.loader_config)
            .ctx(dctx!(), "Failed to parse snapshot loader config")?;
        let loader_path = loader_conf
            .path_string()
            .ctx(dctx!(), "Failed to get loader config path string")?;
        let entry_config = EntryConfigFile::new(&snapshot.selected_entry, &snapshot.entry_config)
            .ctx(dctx!(), "Failed to parse snapshot entry config")?;
        let entry_path = entry_config
            .path_string()
            .ctx(dctx!(), "Failed to get entry config path string")?;

        let configs = vec![
            BootkitConfigRaw {
                file_path: loader_path,
                file: BootkitRawFile::SystemdBootLoader {
                    values: loader_conf.config_file().as_raw_values(),
                },
            },
            BootkitConfigRaw {
                file_path: entry_path,
                file: BootkitRawFile::SystemdBootEntry {
                    values: entry_config.config_file().as_raw_values(),
                },
            },
        ];

        Ok(BootkitConfigsRaw { configs })
    }
}
