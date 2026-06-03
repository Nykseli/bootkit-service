use sqlx::{Pool, Sqlite};

use crate::{
    bootloader::systemd_boot::{boot_entries::SystemdBootEntry, loader_config::LoaderConfigFile},
    data::{
        types::{BootkitBootEntries, BootkitConfig},
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

        let loader_conf = LoaderConfigFile::new(&snapshot.loader_config)
            .ctx(dctx!(), "Failed to parse snapshot loader config")?;

        let timeout = loader_conf
            .get_key_value("timeout")
            .map(|kv| kv.value.clone());
        let bootentries =
            SystemdBootEntry::new().ctx(dctx!(), "Failed to get systemd-boot bootentries")?;

        // TODO: difference between system's selected entry and snapshot entry
        //       should be reported to user as it's not expected behavior
        let selected_boot = snapshot.selected_kernel.or(bootentries.selected);

        Ok(BootkitConfig {
            timeout,
            kernel_parameters: None,
            boot_entries: BootkitBootEntries {
                selected: selected_boot,
                boot_entries: bootentries.entries,
            },
        })
    }
}
