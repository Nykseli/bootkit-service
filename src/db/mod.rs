use sqlx::{sqlite::SqlitePoolOptions, Error, Pool, Sqlite};

use crate::{
    config::{DATABASE_PATH, GRUB_FILE_PATH},
    db::grub2::Grub2Snapshot,
    dctx,
    errors::{DRes, DResult},
    grub2::GrubFile,
};

mod grub2;

#[derive(Clone)]
pub struct Database {
    pool: Pool<Sqlite>,
}

impl Database {
    pub async fn new() -> DResult<Self> {
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

    pub async fn initialize(&self) -> DResult<()> {
        let grub_table = sqlx::query!(
            "SELECT name FROM sqlite_master WHERE type='table' AND name='grub2_snapshot'"
        )
        .fetch_one(&self.pool)
        .await;

        if let Err(Error::RowNotFound) = grub_table {
            log::debug!("grub2_snapshot table not found from database, creating it");
            sqlx::query(include_str!("../../db/grub2.sql"))
                .execute(&self.pool)
                .await
                .ctx(dctx!(), "Cannot initialize grub2_snapshots")?;

            log::debug!("Setting first entry to grub2_snapshots");
            // TODO: get selected kernel from somewhere
            let grub = GrubFile::from_file(GRUB_FILE_PATH)?;
            self.save_grub2(&grub).await?;
        }

        log::info!("Initialised database at {DATABASE_PATH}");
        Ok(())
    }

    pub async fn save_grub2(&self, grub: &GrubFile) -> DResult<()> {
        // TODO: save selected kernel as well
        let grub_file = grub.as_string();

        sqlx::query!(
            "INSERT INTO grub2_snapshot (grub_config) VALUES (?)",
            grub_file
        )
        .execute(&self.pool)
        .await
        .ctx(dctx!(), "Cannot insert new entry to grub2_snapshot table")?;

        log::debug!("New grub2 config snapshot inserted to grub2_snapshot table");
        Ok(())
    }

    pub async fn latest_grub2(&self) -> DResult<Grub2Snapshot> {
        let snapshot = sqlx::query_as!(
            Grub2Snapshot,
            "SELECT * FROM grub2_snapshot ORDER BY id DESC LIMIT 1",
        )
        .fetch_one(&self.pool)
        .await
        .ctx(dctx!(), "Cannot fetch snapshot from grub2_snapshot table")?;

        Ok(snapshot)
    }
}
