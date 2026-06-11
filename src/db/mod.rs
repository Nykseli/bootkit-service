use std::{fs::File, marker::PhantomData, path::Path};

use sqlx::{sqlite::SqlitePoolOptions, Pool, Sqlite};

use crate::{
    bootloader::BootloaderType,
    config::DATABASE_PATH,
    db::{grub2::initialize_grub2_database, setup::SetupDb, systemd_boot::initialize_systemd_boot},
    dctx,
    errors::{DRes, DResult},
};

pub mod grub2;
pub mod selected_snapshot;
mod setup;
pub mod systemd_boot;

#[derive(Clone)]
pub struct InitializedDb {}
#[derive(Clone)]
pub struct UninitializedDb {}

#[derive(Clone)]
pub struct Database<T> {
    pool: Pool<Sqlite>,
    _type: PhantomData<T>,
}

impl Database<UninitializedDb> {
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

        Ok(Self {
            pool,
            _type: PhantomData,
        })
    }

    pub async fn initialize(self) -> DResult<Database<InitializedDb>> {
        let setup = SetupDb::new(&self.pool);
        setup
            .setup_tables()
            .await
            .ctx(dctx!(), "Failed to setup database")?;

        match BootloaderType::system_type() {
            BootloaderType::Grub => initialize_grub2_database(&self.pool)
                .await
                .ctx(dctx!(), "Grub db initialization failed"),
            BootloaderType::SystemdBoot => initialize_systemd_boot(&self.pool)
                .await
                .ctx(dctx!(), "Systemd boot db initialization failed"),
        }?;

        Ok(Database::<InitializedDb> {
            pool: self.pool,
            _type: PhantomData,
        })
    }
}

impl Database<InitializedDb> {
    pub fn pool(&self) -> &Pool<Sqlite> {
        &self.pool
    }
}
