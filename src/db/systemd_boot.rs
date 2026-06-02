use chrono::NaiveDateTime;
use serde::Serialize;
use sqlx::{Pool, Sqlite};

use crate::{
    bootloader::systemd_boot::loader_config::LoaderConfigFile,
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
    /// selected kernel that's booted to, if it's actually specified
    pub selected_kernel: Option<String>,
    /// when snapshot was created
    pub created: NaiveDateTime,
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
        if cfg!(feature = "dev") {
            log::debug!("Setting initial snapshot without selected kernel");
            save_systemd_boot(pool, &config, None::<&str>)
                .await
                .ctx(dctx!(), "Failed to save systemd-boot enry")?;
        } else {
            log::warn!("Setting selected bootentry for systemd-boot is not supported yet");
            save_systemd_boot(pool, &config, None::<&str>)
                .await
                .ctx(dctx!(), "Failed to save systemd-boot enry")?;
        }
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

pub async fn save_systemd_boot<K: Into<String>>(
    pool: &Pool<Sqlite>,
    conf: &LoaderConfigFile,
    selected_kernel: Option<K>,
) -> DResult<()> {
    let selected_kernel: Option<String> = selected_kernel.map(K::into);
    let grub_file = conf.as_string();

    sqlx::query!(
        "INSERT INTO systemd_boot_snapshot (loader_config, selected_kernel) VALUES (?, ?)",
        grub_file,
        selected_kernel,
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
