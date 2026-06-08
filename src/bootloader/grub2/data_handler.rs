use std::{collections::HashMap, process::Command};

use similar::TextDiff;

use crate::{
    bootloader::{
        grub2::{boot_entries::Grub2BootEntries, config_file::Grub2ConfigFile},
        parser::ConfigFileParser,
    },
    config::GRUB_FILE_PATH,
    data::{
        types::{
            BootkitBootEntries, BootkitBootEntry, BootkitConfig, BootkitSnapshot,
            BootkitSnapshotConfig, BootkitSnapshotSelect, BootkitSnapshots,
        },
        BootkitDataHandler,
    },
    db::Database,
    dctx,
    errors::{DError, DRes, DResult},
};

fn set_default_kernel<K: AsRef<str>>(kernel_entry: K) -> DResult<()> {
    let kernel_entry = kernel_entry.as_ref();
    log::debug!("Calling grub2-set-default {kernel_entry}");

    let set_default = Command::new("grub2-set-default")
        .arg(kernel_entry)
        .output()
        .ctx(dctx!(), "Failed to read output from grub2-set-default")?;

    log::trace!(
        "grub2-set-default stdout: {}",
        String::from_utf8_lossy(&set_default.stdout)
    );
    log::trace!(
        "grub2-set-default stderr: {}",
        String::from_utf8_lossy(&set_default.stderr)
    );

    log::debug!("Calling grub2-set-default {kernel_entry}, done");
    Ok(())
}

fn unset_default_kernel() -> DResult<()> {
    log::debug!("Removing default seleceted kernel");

    // grub2-editenv /boot/grub2/grubenv unset saved_entry
    let edit_env = Command::new("grub2-editenv")
        .arg("/boot/grub2/grubenv")
        .arg("unset")
        .arg("saved_entry")
        .output()
        .ctx(dctx!(), "Failed to read output from grub2-editenv")?;

    log::trace!(
        "grub2-edit-env stdout: {}",
        String::from_utf8_lossy(&edit_env.stdout)
    );
    log::trace!(
        "grub2-edit-env stderr: {}",
        String::from_utf8_lossy(&edit_env.stderr)
    );

    log::debug!("Removing default seleceted kernel done");
    Ok(())
}

fn update_grub2_config(grub_config: &Grub2ConfigFile) -> DResult<()> {
    grub_config
        .save()
        .ctx(dctx!(), "Failed to save grub config")?;

    log::debug!("Grub2 config was written to {GRUB_FILE_PATH}");

    log::debug!("Calling grub2-mkconfig -o /boot/grub2/grub.cfg");
    let mkconfig_child = Command::new("grub2-mkconfig")
        .arg("-o")
        .arg("/boot/grub2/grub.cfg")
        .output()
        .ctx(dctx!(), "Failed to read output from grub2-mkconfig")?;

    log::trace!(
        "grub2-mkconfig stdout: {}",
        String::from_utf8(mkconfig_child.stdout).unwrap()
    );
    log::trace!(
        "grub2-mkconfig stderr: {}",
        String::from_utf8(mkconfig_child.stderr).unwrap()
    );

    log::debug!("Calling grub2-mkconfig -o /boot/grub2/grub.cfg done");
    Ok(())
}

fn update_grub2_system_cfg(
    grub_config: &mut Grub2ConfigFile,
    selected_kernel: &Option<String>,
    // TODO: from_snapshot
) -> DResult<()> {
    if let Some(kernel) = selected_kernel {
        let entries = Grub2BootEntries::new().ctx(dctx!(), "Failed to get grub boot entries")?;
        let entry = entries
            .entries()
            .iter()
            .find(|entry| entry.entry() == kernel)
            .ctx(
                dctx!(),
                format!("Kernel entry '{kernel}' is not found from grub configs"),
            )?;

        set_default_kernel(entry.full_path()).ctx(dctx!(), "Failed to set default kernel")?;

        // make sure GRUB_DEFAULT is set to saved as it's required by grub
        grub_config.update_or_insert("GRUB_DEFAULT", "saved");
    } else {
        unset_default_kernel().ctx(dctx!(), "Failed to unset default kernel")?;
    }

    update_grub2_config(grub_config).ctx(dctx!(), "Failed to update grub2 config")?;

    Ok(())
}

#[derive(Clone)]
pub struct Grub2DataHandler {
    // TODO: rewrite this into grub2 database
    db: Database,
}

impl Grub2DataHandler {
    pub fn new(db: Database) -> Self {
        Self { db }
    }
}

impl BootkitDataHandler for Grub2DataHandler {
    async fn get_config(&self) -> DResult<BootkitConfig> {
        let grub = Grub2ConfigFile::from_file(GRUB_FILE_PATH)
            .ctx(dctx!(), "Failed to parse default grub config")?;
        let boot_entries =
            Grub2BootEntries::new().ctx(dctx!(), "Failed to get grub2 boot entries")?;

        let selected = self.db.selected_snapshot().await?;
        let selected_grub = if let Some(id) = selected.grub2_snapshot_id {
            self.db
                .grub2_snapshot(id)
                .await
                .ctx(dctx!(), "Failed to get grub snapshot")?
        } else {
            self.db
                .latest_grub2()
                .await
                .ctx(dctx!(), "Failed to get current grub snapshot")?
        };

        // TODO: replace this with diffing in parser
        let diff = TextDiff::from_lines(&selected_grub.grub_config, &grub.as_string())
            .unified_diff()
            .to_string();

        let config_diffs = if !diff.trim().is_empty() {
            // TODO: add the potential difference in kernel entries to config_diff as well
            Some(HashMap::from([(GRUB_FILE_PATH.to_string(), diff)]))
        } else {
            None
        };

        let timeout = grub.timeout();
        let kernel_arguments = grub.kernel_arguments();
        let entries = boot_entries
            .entries()
            .iter()
            .map(|entry| BootkitBootEntry {
                name: entry.entry().into(),
                title: None,
            })
            .collect();

        Ok(BootkitConfig {
            timeout,
            config_diffs,
            kernel_arguments,
            boot_entries: BootkitBootEntries {
                selected: boot_entries.selected(),
                boot_entries: entries,
            },
        })
    }

    async fn save_config(&self, config: &BootkitConfig) -> DResult<()> {
        let selected = self.db.selected_snapshot().await?;
        let selected_grub = if let Some(id) = selected.grub2_snapshot_id {
            self.db
                .grub2_snapshot(id)
                .await
                .ctx(dctx!(), "Failed to get grub snapshot")?
        } else {
            self.db
                .latest_grub2()
                .await
                .ctx(dctx!(), "Failed to get current grub snapshot")?
        };

        let mut grub_config = Grub2ConfigFile::new(GRUB_FILE_PATH, &selected_grub.grub_config)
            .ctx(dctx!(), "Failed to parse grub config from snapshot")?;

        if let Some(timeout) = &config.timeout {
            grub_config.set_timeout(timeout);
        }

        if let Some(args) = &config.kernel_arguments {
            grub_config.set_kernel_arguments(args);
        }

        update_grub2_system_cfg(&mut grub_config, &config.boot_entries.selected)
            .ctx(dctx!(), "Failed to update grub configuration")?;

        self.db
            .save_grub2_config(
                grub_config.as_string(),
                config.boot_entries.selected.as_deref(),
            )
            .await
            .ctx(dctx!(), "Failed to save new grub2 snapshot")?;

        // set the selected snapshot to None since we want to use the latest one after saving
        self.db
            .set_selected_snapshot(None)
            .await
            .ctx(dctx!(), "Failed to reset selected grub2 snapshot")?;

        log::debug!("Succesfully saved a new grub2 config");
        Ok(())
    }

    async fn get_snapshots(&self) -> DResult<BootkitSnapshots> {
        let db_snapshots = self
            .db
            .grub2_snapshots()
            .await
            .ctx(dctx!(), "Failed to get grub2 snapshots")?;
        let selected = self
            .db
            .selected_snapshot()
            .await
            .ctx(dctx!(), "Failed to get selected grub2 snapshot")?;
        let grub =
            Grub2ConfigFile::from_file(GRUB_FILE_PATH).ctx(dctx!(), "Failed to read grub file")?;
        let snapshots: Vec<BootkitSnapshot> = db_snapshots
            .into_iter()
            .map(|snapshot| {
                let diff = grub.compare_diff_str(&snapshot.grub_config);
                let grub_config = BootkitSnapshotConfig {
                    diff,
                    contents: snapshot.grub_config,
                };
                let kernel = snapshot.selected_kernel.map(|kernel| BootkitBootEntry {
                    name: kernel,
                    title: None,
                });

                BootkitSnapshot {
                    kernel,
                    id: snapshot.id,
                    created: snapshot.created,
                    configs: HashMap::from([(GRUB_FILE_PATH.into(), grub_config)]),
                }
            })
            .collect();

        Ok(BootkitSnapshots {
            snapshots,
            selected: selected.grub2_snapshot_id,
        })
    }

    async fn select_snapshot(&self, _select: &BootkitSnapshotSelect) -> DResult<()> {
        Err(DError::generic(dctx!(), "Not implemented"))
    }

    async fn use_current_snapshot(&self) -> DResult<()> {
        Err(DError::generic(dctx!(), "Not implemented"))
    }

    async fn remove_snapshot(&self, _select: &BootkitSnapshotSelect) -> DResult<()> {
        Err(DError::generic(dctx!(), "Not implemented"))
    }
}
