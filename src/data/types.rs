use std::collections::HashMap;

use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

use crate::{
    dctx,
    errors::{DRes, DResult},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootkitBootEntry {
    /// "Raw" name, usually containing more techical info
    pub name: String,
    /// Pretty name, if available
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootkitBootEntries {
    pub selected: Option<String>,
    pub boot_entries: Vec<BootkitBootEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootkitConfig {
    pub timeout: Option<String>,
    pub boot_entries: BootkitBootEntries,
    pub kernel_arguments: Option<String>,
    // TODO: Raw config values for the specific bootloader
    // TODO: config diff map "filname" -> "diff data"
}

impl BootkitConfig {
    /// Serialize config to json string
    pub fn serialize(&self) -> DResult<String> {
        serde_json::to_string(self).ctx(dctx!(), "Failed to serialize BootkitConfig")
    }

    /// json string -> BootkitConfig
    pub fn deserialize(json: &str) -> DResult<Self> {
        serde_json::from_str(json).ctx(dctx!(), "Failed to deserialize BootkitConfig")
    }
}

impl Default for BootkitConfig {
    fn default() -> Self {
        Self {
            timeout: Default::default(),
            boot_entries: BootkitBootEntries {
                selected: None,
                boot_entries: vec![],
            },
            kernel_arguments: Default::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootkitSnapshot {
    /// Snapshot id from database
    pub id: i64,
    /// Timestamp from database
    pub created: NaiveDateTime,
    /// Raw bootloader specific config(s)
    /// File name -> config data
    pub configs: HashMap<String, String>,
    /// Selected kernel, None means default
    pub kernel: Option<BootkitBootEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootkitSnapshots {
    /// ID if the selected snapshot. None means latest one is selected
    pub selected: Option<i64>,
    pub snapshots: Vec<BootkitSnapshot>,
    // TODO: Raw config values for the specific bootloader
    // TODO: config diff map "filname" -> "diff data"
}

impl BootkitSnapshots {
    /// Serialize config to json string
    pub fn serialize(&self) -> DResult<String> {
        serde_json::to_string(self).ctx(dctx!(), "Failed to serialize BootkitSnapshot")
    }
}
