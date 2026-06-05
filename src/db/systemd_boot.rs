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
    db::selected_snapshot::SelectedSnapshot,
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
    /// selected entry config id
    pub selected_entry: String,
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

    pub async fn snapshot(&self, id: i64) -> DResult<SystemdBootSnapshot> {
        let snapshot = sqlx::query_as!(
            SystemdBootSnapshot,
            "SELECT * FROM systemd_boot_snapshot WHERE id=(?)",
            id
        )
        .fetch_one(&self.pool)
        .await
        .ctx(
            dctx!(),
            "Cannot fetch snapshot with id '{id}' from systemd_boot_snapshot table",
        )?;

        Ok(snapshot)
    }

    /// Get selected snapshot if seleced snapshot is specified, else return latest snapshot
    pub async fn current_snapshot(&self) -> DResult<SystemdBootSnapshot> {
        let snapshot = sqlx::query_as!(
            SystemdBootSnapshot,
            r#"SELECT id, loader_config, selected_entry, entry_config, created FROM systemd_boot_snapshot
            INNER JOIN selected_snapshot on id =
            CASE WHEN selected_snapshot.systemd_boot_snapshot_id IS NULL THEN id ELSE selected_snapshot.systemd_boot_snapshot_id END
            ORDER BY id DESC LIMIT 1"#
        )
        .fetch_one(&self.pool)
        .await
        .ctx(
            dctx!(),
            "Cannot fetch snapshot from systemd_boot_snapshot table",
        )?;

        Ok(snapshot)
    }

    pub async fn selected_snapshot_id(&self) -> DResult<Option<i64>> {
        let snapshot = sqlx::query_as!(SelectedSnapshot, "SELECT * FROM selected_snapshot",)
            .fetch_one(&self.pool)
            .await
            .ctx(dctx!(), "Cannot fetch selected_snapshot table")?;

        Ok(snapshot.systemd_boot_snapshot_id)
    }

    #[allow(dead_code)]
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

    pub async fn set_selected_snapshot(&self, id: Option<i64>) -> DResult<()> {
        sqlx::query!(
            "UPDATE selected_snapshot SET systemd_boot_snapshot_id=(?)",
            id
        )
        .execute(&self.pool)
        .await
        .ctx(
            dctx!(),
            "Failed to update selected snapshot for systemd-boot",
        )?;

        Ok(())
    }

    pub async fn remove_snapshot(&self, id: i64) -> DResult<()> {
        sqlx::query!("DELETE FROM systemd_boot_snapshot WHERE id=(?)", id)
            .execute(&self.pool)
            .await
            .ctx(dctx!(), "Failed to remove snapshot for systemd-boot")?;

        Ok(())
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
    let selected_entry = entry.id();
    let entry_config = entry.as_string();
    let loader_config = conf.as_string();

    sqlx::query!(
        "INSERT INTO systemd_boot_snapshot (loader_config, selected_entry, entry_config) VALUES (?, ?, ?)",
        loader_config,
        selected_entry,
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
