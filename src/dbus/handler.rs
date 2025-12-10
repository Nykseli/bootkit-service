use std::{fs::File, io::Write, process::Command};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use similar::TextDiff;

use crate::{
    config::GRUB_FILE_PATH,
    db::Database,
    dctx,
    errors::{DRes, DResult},
    grub2::{GrubBootEntries, GrubFile, GrubLine},
};

/// Dbus response structure. Set err to NULL when ok, and ok to NULL when err
#[derive(Debug, Clone, Serialize)]
struct DbusResponse {
    // TODO: make into enum?
    ok: Value,
    err: Value,
}

impl DbusResponse {
    fn as_string(&self) -> String {
        serde_json::to_string(self).expect("Unexpected internal JSON parse error")
    }
}

impl<T: Serialize> From<DResult<T>> for DbusResponse {
    fn from(value: DResult<T>) -> Self {
        let (ok, err) = match value {
            Ok(value) => (
                serde_json::to_value(value).expect("Unexpected internal JSON parse error"),
                Value::Null,
            ),
            Err(err) => (Value::Null, Value::String(err.error().as_string())),
        };

        Self { ok, err }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ConfigData {
    value_map: Value,
    value_list: Value,
    config_diff: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BootEntryData {
    entries: Value,
}

#[derive(Clone)]
pub struct DbusHandler {
    db: Database,
}

impl DbusHandler {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    async fn _get_grub2_config(&self) -> DResult<ConfigData> {
        let grub = GrubFile::from_file(GRUB_FILE_PATH)?;
        let latest = self.db.latest_grub2().await?;
        let diff = TextDiff::from_lines(&latest.grub_config, &grub.as_string())
            .unified_diff()
            .to_string();

        let config_diff = if diff.is_empty() {
            None
        } else {
            Some(Value::String(diff))
        };

        let value_map = serde_json::to_value(grub.keyvalues())
            .ctx(dctx!(), "Cannot turn grub keyvalues into json")?;
        let value_list =
            serde_json::to_value(grub.lines()).ctx(dctx!(), "Cannot turn grub lines into json")?;

        Ok(ConfigData {
            value_list,
            value_map,
            config_diff,
        })
    }

    /// Get grub config config (or the relevant error) that can be safely sent via dbus
    pub async fn get_grub2_config_json(&self) -> String {
        let data: DbusResponse = self._get_grub2_config().await.into();
        data.as_string()
    }

    async fn _save_grub2_config(&self, data: &str) -> DResult<String> {
        let config: ConfigData = serde_json::from_str(data)
            .ctx(dctx!(), "Malformed JSON data received from the client")?;
        let value_list: Vec<GrubLine> = serde_json::from_value(config.value_list)
            .ctx(dctx!(), "Cannot turn json into GrubLines")?;
        let grub_file = GrubFile::from_lines(&value_list);
        let file = grub_file.as_string();

        // TODO: start a background thread that executes the grub config
        //       and return an ID that the client can use to poll information

        // WARN: this triggers FileChanged signal
        let mut grub = File::create(GRUB_FILE_PATH).unwrap();
        write!(grub, "{}", file).unwrap();
        log::debug!("Grub2 config was written to {GRUB_FILE_PATH}");

        log::debug!("Calling grub2-mkconfig -o /boot/grub2/grub.cfg");
        let mkconfig_child = Command::new("grub2-mkconfig")
            .arg("-o")
            .arg("/boot/grub2/grub.cfg")
            .output()
            .ctx(dctx!(), "Failed to read output from grub2-mkconfig")?;

        log::debug!(
            "grub2-mkconfig stdout: {}",
            String::from_utf8(mkconfig_child.stdout).unwrap()
        );
        log::debug!(
            "grub2-mkconfig stderr: {}",
            String::from_utf8(mkconfig_child.stderr).unwrap()
        );

        log::debug!("Calling grub2-mkconfig -o /boot/grub2/grub.cfg done");
        // if everything is okay, save the snapshot to a database
        self.db.save_grub2(&grub_file).await?;

        Ok("ok".into())
    }

    /// Save grub config as a snapshot to db
    pub async fn save_grub2_config(&self, data: &str) -> String {
        let data: DbusResponse = self._save_grub2_config(data).await.into();
        data.as_string()
    }

    async fn _get_grub2_boot_entries(&self) -> DResult<BootEntryData> {
        let grub_entries = GrubBootEntries::new().ctx(dctx!(), "Couldn't read kernel entries")?;
        let entries = serde_json::to_value(grub_entries.entries())
            .ctx(dctx!(), "Cannot trun grub kernel entries into json")?;
        Ok(BootEntryData { entries })
    }

    /// Get grub2 boot entries that can be safely sent via dbus
    pub async fn get_grub2_boot_entries(&self) -> String {
        let data: DbusResponse = self._get_grub2_boot_entries().await.into();
        data.as_string()
    }
}
