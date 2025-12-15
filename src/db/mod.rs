use sqlx::{sqlite::SqlitePoolOptions, Error, Pool, Sqlite};

use crate::{
    config::{DATABASE_PATH, GRUB_FILE_PATH},
    db::{grub2::Grub2Snapshot, selected_snapshot::SelectedSnapshot},
    dctx,
    errors::{DRes, DResult},
    grub2::{GrubBootEntries, GrubFile},
};

pub mod grub2;
pub mod selected_snapshot;

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

            let grub = GrubFile::from_file(GRUB_FILE_PATH)?;
            if cfg!(feature = "dev") {
                log::debug!("Setting initial snapshot without selected kernel");
                self.save_grub2(&grub, None::<&str>).await?;
            } else {
                let entry = GrubBootEntries::new()?;
                self.save_grub2(&grub, entry.selected()).await?;
            }
        }

        let grub_table = sqlx::query!(
            "SELECT name FROM sqlite_master WHERE type='table' AND name='selected_snapshot'"
        )
        .fetch_one(&self.pool)
        .await;

        if let Err(Error::RowNotFound) = grub_table {
            log::debug!("selected_snapshot table not found from database, creating it");
            sqlx::query(include_str!("../../db/selected_snapshot.sql"))
                .execute(&self.pool)
                .await
                .ctx(dctx!(), "Cannot initialize selected_snapshots table")?;
        }

        log::info!("Initialised database at {DATABASE_PATH}");
        Ok(())
    }

    pub async fn save_grub2<K: Into<String>>(
        &self,
        grub: &GrubFile,
        selected_kernel: Option<K>,
    ) -> DResult<()> {
        let selected_kernel: Option<String> = selected_kernel.map(K::into);
        let grub_file = grub.as_string();

        sqlx::query!(
            "INSERT INTO grub2_snapshot (grub_config, selected_kernel) VALUES (?, ?)",
            grub_file,
            selected_kernel,
        )
        .execute(&self.pool)
        .await
        .ctx(dctx!(), "Cannot insert new entry to grub2_snapshot table")?;

        log::debug!("New grub2 config snapshot inserted to grub2_snapshot table");
        Ok(())
    }

    pub async fn remove_grub2(&self, grub_id: i64) -> DResult<()> {
        sqlx::query!("DELETE FROM grub2_snapshot WHERE id=(?)", grub_id)
            .execute(&self.pool)
            .await
            .ctx(dctx!(), "Cannot remove snapshot with id {grub_id}")?;

        log::debug!("Grub2 snapshot with id {grub_id} was removed");
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

    pub async fn grub2_snapshots(&self) -> DResult<Vec<Grub2Snapshot>> {
        let snapshots = sqlx::query_as!(
            Grub2Snapshot,
            "SELECT * FROM grub2_snapshot ORDER BY id DESC",
        )
        .fetch_all(&self.pool)
        .await
        .ctx(dctx!(), "Cannot fetch snapshot from grub2_snapshot table")?;

        Ok(snapshots)
    }

    pub async fn grub2_snapshot(&self, id: i64) -> DResult<Grub2Snapshot> {
        let snapshots = sqlx::query_as!(
            Grub2Snapshot,
            "SELECT * FROM grub2_snapshot WHERE id=(?)",
            id
        )
        .fetch_one(&self.pool)
        .await
        .ctx(
            dctx!(),
            "Cannot fetch snapshot with id '{id}' from grub2_snapshot table",
        )?;

        Ok(snapshots)
    }

    pub async fn selected_snapshot(&self) -> DResult<SelectedSnapshot> {
        let snapshot = sqlx::query_as!(SelectedSnapshot, "SELECT * FROM selected_snapshot",)
            .fetch_one(&self.pool)
            .await
            .ctx(
                dctx!(),
                "Cannot fetch selected snapshot from selected_snapshot table",
            )?;

        Ok(snapshot)
    }

    pub async fn set_selected_snapshot(&self, id: Option<i64>) -> DResult<()> {
        sqlx::query!("UPDATE selected_snapshot SET grub2_snapshot_id=(?)", id)
            .execute(&self.pool)
            .await
            .ctx(dctx!(), "Cannot snapshot from selected snapshot table")?;

        Ok(())
    }
}
