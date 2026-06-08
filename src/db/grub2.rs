use chrono::NaiveDateTime;
use serde::Serialize;
use sqlx::{Error, Pool, Sqlite};

use crate::{
    bootloader::{
        grub2::{boot_entries::Grub2BootEntries, config_file::Grub2ConfigFile},
        parser::ConfigFileParser,
    },
    config::{DATABASE_PATH, GRUB_FILE_PATH},
    db::selected_snapshot::SelectedSnapshot,
    dctx,
    errors::{DRes, DResult},
};

#[derive(Debug, Serialize)]
#[allow(dead_code)]
pub struct Grub2Snapshot {
    /// Auto incrementing snapshot id
    pub id: i64,
    /// /etc/default/grub config
    pub grub_config: String,
    /// selected kernel that's booted to, if it's actually specified
    pub selected_kernel: Option<String>,
    /// when snapshot was created
    pub created: NaiveDateTime,
}

#[derive(Clone)]
pub struct Grub2Db {
    pool: Pool<Sqlite>,
}

impl Grub2Db {
    pub fn new(pool: Pool<Sqlite>) -> Self {
        Self { pool }
    }

    async fn initialize(&self) -> DResult<()> {
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
        }

        let snapshot_count = sqlx::query!("SELECT COUNT(*) as count FROM grub2_snapshot")
            .fetch_one(&self.pool)
            .await
            .ctx(dctx!(), "Cannot get count from grub2_snapshot")?;

        if snapshot_count.count == 0 {
            log::debug!("grub2_snapshot table is empty. Setting first entry to grub2_snapshots");
            let grub = Grub2ConfigFile::from_file(GRUB_FILE_PATH)?;
            if cfg!(feature = "dev") {
                log::debug!("Setting initial snapshot without selected kernel");
                self.save_grub2(&grub, None::<&str>).await?;
            } else {
                let entry = Grub2BootEntries::new()?;
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
        grub: &Grub2ConfigFile,
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

    pub async fn save_grub2_config<K: Into<String>>(
        &self,
        grub_file: String,
        selected_kernel: Option<K>,
    ) -> DResult<()> {
        let selected_kernel: Option<String> = selected_kernel.map(K::into);

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

    pub async fn remove_snapshot(&self, id: i64) -> DResult<()> {
        sqlx::query!("DELETE FROM grub2_snapshot WHERE id=(?)", id)
            .execute(&self.pool)
            .await
            .ctx(dctx!(), "Cannot remove snapshot with id {id}")?;

        log::debug!("Grub2 snapshot with id {id} was removed");
        Ok(())
    }

    #[allow(dead_code)]
    pub async fn latest_snapshot(&self) -> DResult<Grub2Snapshot> {
        let snapshot = sqlx::query_as!(
            Grub2Snapshot,
            "SELECT * FROM grub2_snapshot ORDER BY id DESC LIMIT 1",
        )
        .fetch_one(&self.pool)
        .await
        .ctx(dctx!(), "Cannot fetch snapshot from grub2_snapshot table")?;

        Ok(snapshot)
    }

    pub async fn snapshots(&self) -> DResult<Vec<Grub2Snapshot>> {
        let snapshots = sqlx::query_as!(
            Grub2Snapshot,
            "SELECT * FROM grub2_snapshot ORDER BY id DESC",
        )
        .fetch_all(&self.pool)
        .await
        .ctx(dctx!(), "Cannot fetch snapshot from grub2_snapshot table")?;

        Ok(snapshots)
    }

    pub async fn snapshot(&self, id: i64) -> DResult<Grub2Snapshot> {
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

    /// Get selected snapshot if seleced snapshot is specified, else return latest snapshot
    pub async fn current_snapshot(&self) -> DResult<Grub2Snapshot> {
        let snapshot = sqlx::query_as!(
            Grub2Snapshot,
            r#"SELECT id, grub_config, selected_kernel, created FROM grub2_snapshot
            INNER JOIN selected_snapshot on id =
            CASE WHEN selected_snapshot.grub2_snapshot_id IS NULL THEN id ELSE selected_snapshot.systemd_boot_snapshot_id END
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

pub async fn initialize_grub2_database(pool: &Pool<Sqlite>) -> DResult<()> {
    let db = Grub2Db::new(pool.clone());
    db.initialize()
        .await
        .ctx(dctx!(), "Failed to initialize grub2 database")?;
    Ok(())
}
