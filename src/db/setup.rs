use std::fmt::Display;

use serde::Serialize;
use sqlx::{Error, Pool, Sqlite};

use crate::{
    dctx,
    errors::{DError, DRes, DResult},
};

macro_rules! include_migration {
    ($file:expr) => {
        ($file, include_str!($file))
    };
}

/// Required migrations that are required for versions equal or lower to SEMVER
/// Note that migratsions are run in this order so be careful how you place them
const MIGRATION_LIST: &[(SemVer, &[(&str, &str)])] = &[(
    SemVer::new(0, 5, 0),
    &[include_migration!(
        "../../db/migrations/0001_systemd_selected_snapshot.sql"
    )],
)];

fn get_required_migrations(old_ver: Option<SemVer>) -> Vec<(&'static str, &'static str)> {
    if let Some(ver) = old_ver {
        let start = MIGRATION_LIST
            .iter()
            .enumerate()
            .find(|(_, migration)| migration.0 > ver);

        if let Some((start, _)) = start {
            let mut all = Vec::new();
            for migration in &MIGRATION_LIST[start..] {
                all.extend_from_slice(migration.1);
            }
            all
        } else {
            vec![]
        }
    } else {
        let mut all = Vec::new();
        for migration in MIGRATION_LIST {
            all.extend_from_slice(migration.1);
        }
        all
    }
}

#[derive(Debug, Clone, Copy, Serialize, Eq, PartialEq)]
struct SemVer {
    major: u32,
    minor: u32,
    patch: u32,
}

impl SemVer {
    const fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    fn current_string() -> String {
        env!("CARGO_PKG_VERSION").into()
    }
}

impl Display for SemVer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl PartialOrd for SemVer {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match self.major.partial_cmp(&other.major) {
            Some(core::cmp::Ordering::Equal) => {}
            ord => return ord,
        }
        match self.minor.partial_cmp(&other.minor) {
            Some(core::cmp::Ordering::Equal) => {}
            ord => return ord,
        }
        self.patch.partial_cmp(&other.patch)
    }
}

impl TryFrom<&str> for SemVer {
    type Error = DError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let mut split = value.split('.');
        let major = split
            .next()
            .ctx(dctx!(), "Exected major in SemVer string")?
            .parse::<u32>()
            .ctx(dctx!(), "Expected major to be valid u32 in SymVer string")?;
        let minor = split
            .next()
            .ctx(dctx!(), "Exected minor in SemVer string")?
            .parse::<u32>()
            .ctx(dctx!(), "Expected minor to be valid u32 in SymVer string")?;
        let patch = split
            .next()
            .ctx(dctx!(), "Exected patch in SemVer string")?
            .parse::<u32>()
            .ctx(dctx!(), "Expected patch to be valid u32 in SymVer string")?;

        Ok(Self {
            major,
            minor,
            patch,
        })
    }
}

impl TryFrom<String> for SemVer {
    type Error = DError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let res: Result<Self, Self::Error> = value.as_str().try_into();
        res.ctx(dctx!(), "Failed to turn string into SemVer")
    }
}

#[derive(Debug, Serialize)]
#[allow(dead_code)]
struct BootkitVersionTable {
    version: Option<String>,
}

struct Migrations<'a> {
    pool: &'a Pool<Sqlite>,
}

impl<'a> Migrations<'a> {
    fn new(pool: &'a Pool<Sqlite>) -> Self {
        Self { pool }
    }

    /// Get (old) version of bootkit and return it
    /// New version should be set after migrations have been ran
    /// None result means version hasn't yet been setup
    async fn setup_and_get_version(&self) -> DResult<Option<SemVer>> {
        let grub_table = sqlx::query!(
            "SELECT name FROM sqlite_master WHERE type='table' AND name='bootkit_version'"
        )
        .fetch_one(self.pool)
        .await;

        if let Err(Error::RowNotFound) = grub_table {
            log::debug!("bootkit_version table not found from database, creating it");
            sqlx::query(include_str!("../../db/bootkit_version.sql"))
                .execute(self.pool)
                .await
                .ctx(dctx!(), "Cannot initialize bootkit_version table")?;
        }

        let version_table = sqlx::query_as!(BootkitVersionTable, "SELECT * FROM bootkit_version")
            .fetch_one(self.pool)
            .await
            .ctx(dctx!(), "Cannot fetch from bootkit_version")?;

        if let Some(version) = version_table.version {
            let ver: DResult<SemVer> = version.try_into();
            let ver = ver.ctx(dctx!(), "Failed to turn string into SemVer");
            Ok(Some(ver?))
        } else {
            Ok(None)
        }
    }

    async fn set_curent_version(&self) -> DResult<()> {
        let semver = SemVer::current_string();
        sqlx::query!("UPDATE bootkit_version SET version=(?)", semver)
            .execute(self.pool)
            .await
            .ctx(dctx!(), "Cannot set current bootkit version")?;

        Ok(())
    }

    async fn run_migrations(&self) -> DResult<()> {
        let semver = self
            .setup_and_get_version()
            .await
            .ctx(dctx!(), "Failed to setup boot kit version")?;

        log::trace!("Found old semver: {semver:?}");

        let migrations = get_required_migrations(semver);
        for migration in migrations {
            log::debug!("Running migration: {}", migration.0);
            sqlx::query(migration.1)
                .execute(self.pool)
                .await
                .ctx(dctx!(), "Failed to run migration")?;
        }

        self.set_curent_version()
            .await
            .ctx(dctx!(), "Failed to update bootkit version")?;
        Ok(())
    }
}

pub struct SetupDb<'a> {
    pool: &'a Pool<Sqlite>,
}

impl<'a> SetupDb<'a> {
    pub fn new(pool: &'a Pool<Sqlite>) -> Self {
        Self { pool }
    }

    /// setup base table without migrations
    async fn setup_grub2_snapshot(&self) -> DResult<()> {
        let grub_table = sqlx::query!(
            "SELECT name FROM sqlite_master WHERE type='table' AND name='grub2_snapshot'"
        )
        .fetch_one(self.pool)
        .await;

        if let Err(Error::RowNotFound) = grub_table {
            log::debug!("grub2_snapshot table not found from database, creating it");
            sqlx::query(include_str!("../../db/grub2.sql"))
                .execute(self.pool)
                .await
                .ctx(dctx!(), "Cannot initialize grub2_snapshots")?;
        }

        Ok(())
    }

    /// setup base table without migrations
    async fn setup_systemd_boot_snapshot(&self) -> DResult<()> {
        let systemd_table = sqlx::query!(
            "SELECT name FROM sqlite_master WHERE type='table' AND name='systemd_boot_snapshot'"
        )
        .fetch_one(self.pool)
        .await;

        if let Err(Error::RowNotFound) = systemd_table {
            log::debug!("systemd_boot_snapshot table not found from database, creating it");
            sqlx::query(include_str!("../../db/systemd_boot.sql"))
                .execute(self.pool)
                .await
                .ctx(dctx!(), "Cannot initialize systemd_boot_snapshot")?;
        }
        Ok(())
    }

    /// setup base table without migrations
    async fn setup_selected_snapshot(&self) -> DResult<()> {
        let snapshot_table = sqlx::query!(
            "SELECT name FROM sqlite_master WHERE type='table' AND name='selected_snapshot'"
        )
        .fetch_one(self.pool)
        .await;

        if let Err(Error::RowNotFound) = snapshot_table {
            log::debug!("selected_snapshot table not found from database, creating it");
            sqlx::query(include_str!("../../db/selected_snapshot.sql"))
                .execute(self.pool)
                .await
                .ctx(dctx!(), "Cannot initialize selected_snapshots table")?;
        }
        Ok(())
    }

    /// Setup db tables and the required migrations
    pub async fn setup_tables(&self) -> DResult<()> {
        self.setup_selected_snapshot()
            .await
            .ctx(dctx!(), "Failed to setup selected snapshot table")?;
        self.setup_grub2_snapshot()
            .await
            .ctx(dctx!(), "Failed to setup grub2 snapshot table")?;
        self.setup_systemd_boot_snapshot()
            .await
            .ctx(dctx!(), "Failed to setup systemd-boot snapshot table")?;

        let migrations = Migrations::new(self.pool);
        migrations
            .run_migrations()
            .await
            .ctx(dctx!(), "Failed to run migrations")?;

        Ok(())
    }
}
