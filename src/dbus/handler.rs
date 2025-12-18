use std::{fs::File, io::Write, process::Command};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use similar::TextDiff;

use crate::{
    config::GRUB_FILE_PATH,
    db::{grub2::Grub2Snapshot, selected_snapshot::SelectedSnapshot, Database},
    dctx,
    errors::{DError, DErrorType, DRes, DResult},
    grub2::{GrubBootEntries, GrubFile, GrubLine},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ConfigData {
    value_map: Value,
    value_list: Value,
    config_diff: Option<Value>,
    selected_kernel: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BootEntryData {
    entries: Value,
    selected_kernel: Value,
}

#[derive(Debug, Serialize)]
struct Grub2SnapshotData {
    /// snapshot in the database
    snapshot: Grub2Snapshot,
    /// diff against the current config
    diff: Option<String>,
}

#[derive(Debug, Serialize)]
struct SnapshotData {
    snapshots: Vec<Grub2SnapshotData>,
    selected: SelectedSnapshot,
}

#[derive(Debug, Deserialize, Serialize)]
struct RemoveSnapshotData {
    snapshot_id: i64,
}

#[derive(Debug, Deserialize, Serialize)]
struct SelectSnapshotData {
    snapshot_id: i64,
}

#[derive(Clone)]
pub struct DbusHandler {
    db: Database,
}

impl DbusHandler {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    async fn set_grub_system(
        &self,
        grub_file: &mut GrubFile,
        selected_kernel: &Option<String>,
        from_snapshot: bool,
    ) -> DResult<()> {
        if let Some(kernel) = &selected_kernel {
            let kernel_entries = GrubBootEntries::new()?;
            let kernel_entry = if let Some(entry) = kernel_entries
                .entries()
                .iter()
                .find(|entry| entry.entry() == kernel)
            {
                entry.full_path()
            } else {
                return Err(DError::new(
                    dctx!(),
                    DErrorType::Error(format!(
                        "Kernel entry '{kernel}' is not found from grub configs"
                    )),
                ));
            };

            log::debug!("Calling grub2-set-default {kernel_entry}");

            let set_default = Command::new("grub2-set-default")
                .arg(&kernel_entry)
                .output()
                .ctx(dctx!(), "Failed to read output from grub2-set-default")?;

            log::debug!(
                "grub2-set-default stdout: {}",
                String::from_utf8_lossy(&set_default.stdout)
            );
            log::debug!(
                "grub2-set-default stderr: {}",
                String::from_utf8_lossy(&set_default.stderr)
            );

            log::debug!("Calling grub2-set-default {kernel_entry}, done");

            // Only update grub file when selecting a snapshot
            // old snapshots should always be set back the way they were
            if !from_snapshot {
                // make sure GRUB_DEFAULT is set to saved as it's required by grub
                grub_file.set_key_value("GRUB_DEFAULT", "saved");
            }
        } else {
            log::debug!("Removing default seleceted kernel");

            // grub2-editenv /boot/grub2/grubenv unset saved_entry
            let edit_env = Command::new("grub2-editenv")
                .arg("/boot/grub2/grubenv")
                .arg("unset")
                .arg("saved_entry")
                .output()
                .ctx(dctx!(), "Failed to read output from grub2-editenv")?;

            log::debug!(
                "grub2-edit-env stdout: {}",
                String::from_utf8_lossy(&edit_env.stdout)
            );
            log::debug!(
                "grub2-edit-env stderr: {}",
                String::from_utf8_lossy(&edit_env.stderr)
            );

            log::debug!("Removing default seleceted kernel done");
        }

        let file = grub_file.as_string();

        // TODO: start a background thread that executes the grub config
        //       and return an ID that the client can use to poll information

        // WARN: this triggers FileChanged signal
        let mut grub = File::create(GRUB_FILE_PATH).ctx(
            dctx!(),
            format!("Failed to create grub config in path '{GRUB_FILE_PATH}'"),
        )?;
        write!(grub, "{}", file).ctx(
            dctx!(),
            format!("Failed override grub config in path '{GRUB_FILE_PATH}'"),
        )?;
        log::debug!("Grub2 config was written to {GRUB_FILE_PATH}");

        log::debug!("Calling grub2-mkconfig -o /boot/grub2/grub.cfg");
        let mkconfig_child = Command::new("grub2-mkconfig")
            .arg("-o")
            .arg("/boot/grub2/grub.cfg")
            .output()
            .ctx(dctx!(), "Failed to read output from grub2-mkconfig")?;

        log::debug!(
            "grub2-mkconfig stdout: {}",
            String::from_utf8(mkconfig_child.stdout).unwrap()
        );
        log::debug!(
            "grub2-mkconfig stderr: {}",
            String::from_utf8(mkconfig_child.stderr).unwrap()
        );

        log::debug!("Calling grub2-mkconfig -o /boot/grub2/grub.cfg done");

        Ok(())
    }

    async fn _get_grub2_config(&self) -> DResult<ConfigData> {
        let grub = GrubFile::from_file(GRUB_FILE_PATH)?;
        let kernel_entries = GrubBootEntries::new()?;
        let selected = self.db.selected_snapshot().await?;
        let selected_grub = if let Some(id) = selected.grub2_snapshot_id {
            self.db.grub2_snapshot(id).await?
        } else {
            self.db.latest_grub2().await?
        };

        let diff = TextDiff::from_lines(&selected_grub.grub_config, &grub.as_string())
            .unified_diff()
            .to_string();

        // TODO: add the potential difference in kernel entries to config_diff as well
        let config_diff = if diff.is_empty() {
            None
        } else {
            Some(Value::String(diff))
        };

        let value_map = serde_json::to_value(grub.keyvalues())
            .ctx(dctx!(), "Cannot turn grub keyvalues into json")?;
        let value_list =
            serde_json::to_value(grub.lines()).ctx(dctx!(), "Cannot turn grub lines into json")?;

        Ok(ConfigData {
            value_list,
            value_map,
            config_diff,
            selected_kernel: kernel_entries.selected().map(str::to_string),
        })
    }

    /// Get grub config config (or the relevant error) that can be safely sent via dbus
    pub async fn get_grub2_config_json(&self) -> DResult<String> {
        let data = self._get_grub2_config().await?;
        serde_json::to_string(&data).ctx(dctx!(), "Failed to serialize grub2 config")
    }

    pub async fn save_grub2_config(&self, data: &str) -> DResult<String> {
        let config: ConfigData = serde_json::from_str(data)
            .ctx(dctx!(), "Malformed JSON data received from the client")?;
        let value_list: Vec<GrubLine> = serde_json::from_value(config.value_list)
            .ctx(dctx!(), "Cannot turn json into GrubLines")?;

        let mut grub_file = GrubFile::from_lines(&value_list);
        self.set_grub_system(&mut grub_file, &config.selected_kernel, false)
            .await?;

        // if everything is okay, save the snapshot to a database
        self.db
            .save_grub2(&grub_file, config.selected_kernel)
            .await?;
        // latest snapshot should be null so it's assumed that latest snapshot is selected
        self.db.set_selected_snapshot(None).await?;

        Ok("ok".into())
    }

    async fn _get_grub2_boot_entries(&self) -> DResult<BootEntryData> {
        let grub_entries = GrubBootEntries::new().ctx(dctx!(), "Couldn't read kernel entries")?;
        let entries = serde_json::to_value(grub_entries.entry_names())
            .ctx(dctx!(), "Cannot trun grub kernel entries into json")?;
        let selected_kernel = serde_json::to_value(grub_entries.selected())
            .ctx(dctx!(), "Cannot trun grub kernel entries into json")?;

        Ok(BootEntryData {
            entries,
            selected_kernel,
        })
    }

    /// Get grub2 boot entries that can be safely sent via dbus
    pub async fn get_grub2_boot_entries_json(&self) -> DResult<String> {
        let data = self._get_grub2_boot_entries().await?;
        serde_json::to_string(&data).ctx(dctx!(), "Failed to serialize grub2 bootentries")
    }

    /// Get snapshots that can be safely sent via dbus
    async fn _get_snapshots(&self) -> DResult<SnapshotData> {
        let db_snapshots = self.db.grub2_snapshots().await?;
        let selected = self.db.selected_snapshot().await?;
        let grub = GrubFile::from_file(GRUB_FILE_PATH).ctx(dctx!(), "Failed to read grub file")?;
        let current = grub.as_string();
        let snapshots: Vec<Grub2SnapshotData> = db_snapshots
            .into_iter()
            .map(|snapshot| {
                let diff = TextDiff::from_lines(&current, &snapshot.grub_config)
                    .unified_diff()
                    .to_string();

                let diff = if diff.trim().is_empty() {
                    None
                } else {
                    Some(diff)
                };

                Grub2SnapshotData { snapshot, diff }
            })
            .collect();

        Ok(SnapshotData {
            snapshots,
            selected,
        })
    }

    /// Get snapshots that can be safely sent via dbus
    pub async fn get_snapshots_json(&self) -> DResult<String> {
        let data = self._get_snapshots().await?;
        serde_json::to_string(&data).ctx(dctx!(), "Failed to serialize snapshots")
    }

    pub async fn remove_snapshot(&self, data: &str) -> DResult<String> {
        let rm_data: RemoveSnapshotData =
            serde_json::from_str(data).ctx(dctx!(), "Malformed JSON data received from client")?;

        log::debug!("Trying to remove snapshot with id {}", rm_data.snapshot_id);

        // Don't allow deleting the selected snapshot so things don't get confusing
        let selected = self.db.selected_snapshot().await?;
        let selected_id = if let Some(id) = selected.grub2_snapshot_id {
            id
        } else {
            self.db.latest_grub2().await?.id
        };

        if rm_data.snapshot_id == selected_id {
            return Err(DError::generic(
                dctx!(),
                "Cannot remove currently selected snapshot",
            ));
        }

        self.db.remove_grub2(rm_data.snapshot_id).await?;

        log::debug!(
            "Succesfully removed snapshot with id {}",
            rm_data.snapshot_id
        );
        Ok("ok".into())
    }

    pub async fn select_snapshot(&self, data: &str) -> DResult<String> {
        let select_data: SelectSnapshotData =
            serde_json::from_str(data).ctx(dctx!(), "Malformed JSON data received from client")?;

        log::debug!(
            "Trying to select snapshot with id {}",
            select_data.snapshot_id
        );

        // Don't allow reselecting the selected snapshot so things don't get confusing
        let selected = self.db.selected_snapshot().await?;
        let selected_id = if let Some(id) = selected.grub2_snapshot_id {
            id
        } else {
            self.db.latest_grub2().await?.id
        };

        if select_data.snapshot_id == selected_id {
            return Err(DError::generic(
                dctx!(),
                "Cannot reselect currently selected snapshot",
            ));
        }

        let snapshot = self.db.grub2_snapshot(select_data.snapshot_id).await?;
        let mut grub_file = GrubFile::new(&snapshot.grub_config)?;
        self.set_grub_system(&mut grub_file, &snapshot.selected_kernel, true)
            .await?;
        self.db
            .set_selected_snapshot(Some(select_data.snapshot_id))
            .await?;

        log::debug!(
            "Succesfully selected snapshot with id {}",
            select_data.snapshot_id
        );

        Ok("ok".into())
    }
}
