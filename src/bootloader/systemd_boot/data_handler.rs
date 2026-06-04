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
            BootkitBootEntries, BootkitBootEntry, BootkitConfig, BootkitSnapshot, BootkitSnapshots,
        },
        BootkitDataHandler,
    },
    db::systemd_boot::SystemdDb,
    dctx,
    errors::{DRes, DResult},
};

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
}

impl BootkitDataHandler for SystemdDataHandler {
    async fn get_config(&self) -> DResult<BootkitConfig> {
        let snapshot = self
            .db
            .latest_snapshot()
            .await
            .ctx(dctx!(), "Failed to fetch latest snapshot")?;

        let loader_conf = LoaderConfigFile::new(SYSTEMD_CFG_PATH, &snapshot.loader_config)
            .ctx(dctx!(), "Failed to parse snapshot loader config")?;

        let timeout = loader_conf
            .get_key_value("timeout")
            .map(|kv| kv.value.clone());
        let bootentries =
            SystemdBootEntries::new().ctx(dctx!(), "Failed to get systemd-boot bootentries")?;

        // TODO: difference between system's selected entry and snapshot entry
        //       should be reported to user as it's not expected behavior
        let kernel_arguments = snapshot
            .kernel_arguments
            .or(bootentries.selected.options().map(str::to_string));
        let selected_entry = snapshot.selected_entry;

        let entries = bootentries
            .entries
            .iter()
            .map(|entry| BootkitBootEntry {
                name: entry.name().into(),
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

        let snapshots = snapshots
            .into_iter()
            .map(|snapshot| BootkitSnapshot {
                id: snapshot.id,
                created: snapshot.created,
                config: snapshot.loader_config,
                // TODO: read boot entry info to get more kernel data
                kernel: Some(BootkitBootEntry {
                    name: snapshot.selected_entry,
                    title: None,
                }),
            })
            .collect();

        Ok(BootkitSnapshots {
            snapshots,
            // TODO: get selected
            selected: None,
        })
    }

    async fn save_config(&self, config: &BootkitConfig) -> DResult<()> {
        log::debug!("Start saving sytemd-boot config snapshot");
        log::trace!("Config: {config:?}");
        // TODO: if any of this fails, revert to original configs
        // TODO: check if the config has actual changes to avoid duplicate snapshots

        // TODO: get selected snapshot if one is selected
        let snapshot = self
            .db
            .latest_snapshot()
            .await
            .ctx(dctx!(), "Failed to fetch latest snapshot")?;

        // TODO: set/save selected kernel!

        // TODO: Can we recover from kernel not being selected?
        // TODO: add the assumption to DB that there's always a selected kernel entry
        // TODO: use selected kernel from config, if none (Default), use the current one
        let selected_entry = snapshot.selected_entry;
        let mut entry = EntryConfigFile::new(selected_entry, &snapshot.entry_config)
            .ctx(dctx!(), "failed to get systemd boot entry")?;
        entry.update_config(config);
        entry
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
            .save_systemd_boot(&loader_config, &entry)
            .await
            .ctx(dctx!(), "Failed to save systemd-boot snapshot")?;

        log::debug!("Successfully saved sytemd-boot config snapshot");
        Ok(())
    }
}
