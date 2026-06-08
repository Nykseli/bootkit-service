use std::collections::HashMap;

use similar::TextDiff;

use crate::{
    bootloader::{grub2::config_file::Grub2ConfigFile, parser::ConfigFileParser},
    config::GRUB_FILE_PATH,
    data::{
        types::{BootkitBootEntries, BootkitConfig, BootkitSnapshotSelect, BootkitSnapshots},
        BootkitDataHandler,
    },
    db::Database,
    dctx,
    errors::{DError, DRes, DResult},
};

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

        Ok(BootkitConfig {
            timeout,
            config_diffs,
            kernel_arguments,
            boot_entries: BootkitBootEntries {
                selected: None,
                boot_entries: vec![],
            },
        })
    }

    async fn save_config(&self, _config: &BootkitConfig) -> DResult<()> {
        Err(DError::generic(dctx!(), "Not implemented"))
    }

    async fn get_snapshots(&self) -> DResult<BootkitSnapshots> {
        Err(DError::generic(dctx!(), "Not implemented"))
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
