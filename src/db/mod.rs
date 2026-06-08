use std::{fs::File, path::Path};

use sqlx::{sqlite::SqlitePoolOptions, Pool, Sqlite};

use crate::{
    bootloader::BootloaderType,
    config::DATABASE_PATH,
    db::{grub2::initialize_grub2_database, systemd_boot::initialize_systemd_boot},
    dctx,
    errors::{DRes, DResult},
};

pub mod grub2;
pub mod selected_snapshot;
pub mod systemd_boot;

#[derive(Clone)]
pub struct Database {
    pool: Pool<Sqlite>,
}

impl Database {
    pub async fn new() -> DResult<Self> {
        if !Path::new(DATABASE_PATH).exists() {
            log::debug!("Database file in was not found. Creating it in path {DATABASE_PATH}");
            File::create(DATABASE_PATH).ctx(
                dctx!(),
                format!("Cannot create database in path: {DATABASE_PATH}"),
            )?;
        }

        // should this failure be fatal or should the snapshot features
        // just be disabled?
        let pool = SqlitePoolOptions::new()
            .max_connections(10)
            .connect(DATABASE_PATH)
            .await
            .ctx(
                dctx!(),
                format!("Cannot initialize SQLite database in path: {DATABASE_PATH}"),
            )?;

        Ok(Self { pool })
    }

    pub fn pool(&self) -> &Pool<Sqlite> {
        &self.pool
    }

    pub async fn initialize(&self) -> DResult<()> {
        match BootloaderType::system_type() {
            BootloaderType::Grub => initialize_grub2_database(&self.pool)
                .await
                .ctx(dctx!(), "Grub db initialization failed"),
            BootloaderType::SystemdBoot => initialize_systemd_boot(&self.pool)
                .await
                .ctx(dctx!(), "Systemd boot db initialization failed"),
        }
    }
}
