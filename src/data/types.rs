use std::collections::HashMap;

use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use similar::TextDiff;

use crate::{
    dctx,
    errors::{DRes, DResult},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootkitGrub2ConsoleConfig {
    pub graphical_enabled: bool,
    /// Seleceted console resolution or "auto"
    pub console_resolution: String,
    pub console_theme: Option<String>,
    // TODO: use hwinfo --framebuffer to get available resolutions
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "loader")]
pub enum BootkitConsoleConfigs {
    /// SystemdBoot console configs are not supported
    SystemdBoot,
    Grub2(BootkitGrub2ConsoleConfig),
}

impl BootkitConsoleConfigs {
    pub fn as_grub2(&self) -> Option<&BootkitGrub2ConsoleConfig> {
        match self {
            BootkitConsoleConfigs::Grub2(grub2) => Some(grub2),
            _ => None,
        }
    }
}

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
    /// Possible mismatches of currently selected config and system's state.
    /// Usually caused by 3rd party editing configs
    /// (File) name -> diff data
    pub config_diffs: Option<HashMap<String, String>>,
    /// Console configs for loaders that support them
    pub console: Option<BootkitConsoleConfigs>,
    // TODO: Raw config values for the specific bootloader
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
            config_diffs: None,
            console: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootkitSnapshotConfig {
    /// Config file contents
    pub contents: String,
    /// Difference compared to current config, if there's any
    pub diff: Option<String>,
}

impl BootkitSnapshotConfig {
    /// Compare contents to the reference and set the diff if there is any
    pub fn with_diff<C: Into<String>, R: AsRef<str>>(contents: C, reference: R) -> Self {
        let contents = contents.into();
        let reference = reference.as_ref();
        let text_diff = TextDiff::from_lines(contents.as_str(), reference)
            .unified_diff()
            .to_string();

        // TextDiff doesn't have a better API for detecting if the files
        // are identical so checking if the contents are empty is our best guess
        let diff = if text_diff.trim().is_empty() {
            None
        } else {
            Some(text_diff)
        };

        Self { contents, diff }
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
    pub configs: HashMap<String, BootkitSnapshotConfig>,
    /// Selected kernel, None means default
    pub kernel: Option<BootkitBootEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootkitSnapshots {
    /// ID if the selected snapshot. None means latest one is selected
    pub selected: Option<i64>,
    pub snapshots: Vec<BootkitSnapshot>,
}

impl BootkitSnapshots {
    /// Serialize config to json string
    pub fn serialize(&self) -> DResult<String> {
        serde_json::to_string(self).ctx(dctx!(), "Failed to serialize BootkitSnapshot")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootkitSnapshotSelect {
    pub snapshot_id: i64,
}

impl BootkitSnapshotSelect {
    /// json string -> BootkitSnapshotSelect
    pub fn deserialize(json: &str) -> DResult<Self> {
        serde_json::from_str(json).ctx(dctx!(), "Failed to deserialize BootkitSnapshotSelect")
    }
}
