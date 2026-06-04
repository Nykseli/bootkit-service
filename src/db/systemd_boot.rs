use chrono::NaiveDateTime;
use serde::Serialize;
use sqlx::{Pool, Sqlite};

use crate::{
    bootloader::{
        parser::ConfigFileParser,
        systemd_boot::{
            boot_entries::{EntryConfigFile, SystemdBootEntries},
            loader_config::LoaderConfigFile,
        },
    },
    config::{DATABASE_PATH, SYSTEMD_CFG_PATH},
    dctx,
    errors::{DRes, DResult},
};

#[derive(Debug, Serialize)]
#[allow(dead_code)]
pub struct SystemdBootSnapshot {
    /// Auto incrementing snapshot id
    pub id: i64,
    /// /boot/efi/loader/loader.conf config
    pub loader_config: String,
    /// /boot/efi/loader/entries/ config data
    pub entry_config: String,
    /// selected kernel that's booted to, if it's actually specified
    pub selected_kernel: Option<String>,
    /// kernel args for the selected kernel
    /// systemd-boot ties kernel args to the boot entry
    pub kernel_arguments: Option<String>,
    /// when snapshot was created
    pub created: NaiveDateTime,
}

#[derive(Clone)]
pub struct SystemdDb {
    pool: Pool<Sqlite>,
}

impl SystemdDb {
    pub fn new(pool: Pool<Sqlite>) -> Self {
        Self { pool }
    }

    pub async fn snapshots(&self) -> DResult<Vec<SystemdBootSnapshot>> {
        let snapshots = sqlx::query_as!(
            SystemdBootSnapshot,
            "SELECT * FROM systemd_boot_snapshot ORDER BY id DESC",
        )
        .fetch_all(&self.pool)
        .await
        .ctx(
            dctx!(),
            "Cannot fetch snapshots from systemd_boot_snapshot table",
        )?;

        Ok(snapshots)
    }

    pub async fn latest_snapshot(&self) -> DResult<SystemdBootSnapshot> {
        let snapshot = sqlx::query_as!(
            SystemdBootSnapshot,
            "SELECT * FROM systemd_boot_snapshot ORDER BY id DESC LIMIT 1",
        )
        .fetch_one(&self.pool)
        .await
        .ctx(
            dctx!(),
            "Cannot fetch snapshot from systemd_boot_snapshot table",
        )?;

        Ok(snapshot)
    }

    pub async fn save_systemd_boot(
        &self,
        conf: &LoaderConfigFile,
        entry: &EntryConfigFile,
    ) -> DResult<()> {
        // TODO: add save_systemd_boot logic here and remove the logic there
        save_systemd_boot(&self.pool, conf, entry)
            .await
            .ctx(dctx!(), "Saving systemd-boot snapshot failed")
    }
}

pub async fn initialize_systemd_boot(pool: &Pool<Sqlite>) -> DResult<()> {
    let systemd_table = sqlx::query!(
        "SELECT name FROM sqlite_master WHERE type='table' AND name='systemd_boot_snapshot'"
    )
    .fetch_one(pool)
    .await;

    if let Err(sqlx::Error::RowNotFound) = systemd_table {
        log::debug!("systemd_boot_snapshot table not found from database, creating it");
        sqlx::query(include_str!("../../db/systemd_boot.sql"))
            .execute(pool)
            .await
            .ctx(dctx!(), "Cannot initialize systemd_boot_snapshot")?;
    }

    let snapshot_count = sqlx::query!("SELECT COUNT(*) as count FROM systemd_boot_snapshot")
        .fetch_one(pool)
        .await
        .ctx(dctx!(), "Cannot get count from systemd_boot_snapshot")?;

    if snapshot_count.count == 0 {
        log::debug!("systemd_boot_snapshot table is empty. Setting first entry");
        let config = LoaderConfigFile::from_file(SYSTEMD_CFG_PATH)
            .ctx(dctx!(), "Failed to parse systemd config")?;

        let entries = SystemdBootEntries::new()
            .ctx(dctx!(), "Failed to get systemd-boot boot entry information")?;
        let selected = entries.selected.as_file().ctx(
            dctx!(),
            "Expected selected boot entry to not be auto detected entry",
        )?;

        save_systemd_boot(pool, &config, selected)
            .await
            .ctx(dctx!(), "Failed to save systemd-boot enry")?;
    }

    let grub_table = sqlx::query!(
        "SELECT name FROM sqlite_master WHERE type='table' AND name='selected_snapshot'"
    )
    .fetch_one(pool)
    .await;

    if let Err(sqlx::Error::RowNotFound) = grub_table {
        log::debug!("selected_snapshot table not found from database, creating it");
        sqlx::query(include_str!("../../db/selected_snapshot.sql"))
            .execute(pool)
            .await
            .ctx(dctx!(), "Cannot initialize selected_snapshots table")?;
    }

    log::info!("Initialised database at {DATABASE_PATH}");
    Ok(())
}

pub async fn save_systemd_boot(
    pool: &Pool<Sqlite>,
    conf: &LoaderConfigFile,
    entry: &EntryConfigFile,
) -> DResult<()> {
    let selected_kernel = entry.name();
    let kernel_arguments = entry.options();
    let entry_config = entry.as_string();
    let loader_config = conf.as_string();

    sqlx::query!(
        "INSERT INTO systemd_boot_snapshot (loader_config, selected_kernel, kernel_arguments, entry_config) VALUES (?, ?, ?, ?)",
        loader_config,
        selected_kernel,
        kernel_arguments,
        entry_config,
    )
    .execute(pool)
    .await
    .ctx(
        dctx!(),
        "Cannot insert new entry to systemd_boot_snapshot table",
    )?;

    log::debug!("New systemd-boot config snapshot inserted to systemd_boot_snapshot table");
    Ok(())
}
