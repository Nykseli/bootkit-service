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
    pub kernel_parameters: Option<String>,
    // TODO: Raw config values for the specific bootloader
    // TODO: config diff map "filname" -> "diff data"
}

impl BootkitConfig {
    /// Serialize config to json string
    pub fn serialize(&self) -> DResult<String> {
        serde_json::to_string(self).ctx(dctx!(), "Failed to serialize BootkitConfig")
    }
}
