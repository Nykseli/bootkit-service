use regex::Regex;
use std::{fmt::Display, fs::read_to_string};

use crate::{
    config::{GRUB_CFG_PATH, GRUB_ENV_PATH},
    dctx,
    errors::{DError, DRes, DResult},
};

#[derive(Debug)]
enum Grub2EnvValue<'a> {
    /// Index of the bootentry
    Index(usize),
    /// Name of the bootentry
    // Name(String),
    Name(&'a str),
}

impl Display for Grub2EnvValue<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Grub2EnvValue::Index(idx) => write!(f, "{idx}"),
            Grub2EnvValue::Name(name) => write!(f, "{name}"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Grub2BootEntry {
    /// The actual name of the entry
    entry: String,
    /// (nested) submenus
    submenus: Vec<String>,
}

impl Grub2BootEntry {
    fn new(entry: String, submenus: Vec<String>) -> Self {
        Self { entry, submenus }
    }

    fn parse_entries(contents: &str) -> DResult<Vec<Grub2BootEntry>> {
        let mut entries = Vec::new();
        let mut submenus = Vec::new();
        // these are unrecovable error so panic is appropriate
        let entry_re = Regex::new(r"menuentry\s+'([^']+)").expect("Invalid regex");
        let submenu_re = Regex::new(r"submenu\s+'([^']+)").expect("Invalid regex");

        let mut menuentry_open = false;
        for line in contents.lines() {
            let line = line.trim();
            if line.starts_with('}') {
                if menuentry_open {
                    menuentry_open = false;
                } else {
                    submenus.pop();
                }

                continue;
            }

            if line.starts_with("menuentry") {
                menuentry_open = true;
                // TODO: error if this fails
                if let Some(capture) = entry_re.captures(line) {
                    entries.push(Self::new(capture[1].to_string(), submenus.clone()))
                }
            } else if line.starts_with("submenu") {
                // TODO: error if this fails
                if let Some(capture) = submenu_re.captures(line) {
                    submenus.push(capture[1].to_string())
                }
            }
        }

        Ok(entries)
    }

    pub fn entry(&self) -> &str {
        &self.entry
    }

    pub fn full_path(&self) -> String {
        if self.submenus.is_empty() {
            self.entry.clone()
        } else {
            format!("{}>{}", self.submenus.join(">"), self.entry)
        }
    }
}

#[derive(Debug)]
pub struct Grub2BootEntries {
    entries: Vec<Grub2BootEntry>,
    selected: Option<Grub2BootEntry>,
}
impl Grub2BootEntries {
    pub fn new() -> DResult<Self> {
        log::debug!("Reading kenrnel boot entries from {GRUB_CFG_PATH}");
        let config =
            read_to_string(GRUB_CFG_PATH).ctx(dctx!(), format!("Cannot read {GRUB_CFG_PATH}"))?;

        log::debug!("Reading default boot entry from {GRUB_ENV_PATH}");
        let grub_env =
            read_to_string(GRUB_ENV_PATH).ctx(dctx!(), format!("Cannot read {GRUB_ENV_PATH}"))?;

        Self::from_contents(&config, &grub_env)
    }

    fn from_contents(grub_config: &str, grub_env: &str) -> DResult<Self> {
        let entries = Grub2BootEntry::parse_entries(grub_config)?;

        let selected_idx = grub_env
            .lines()
            .find(|line| line.starts_with("saved_entry"))
            .map(|entry| {
                let split = entry.split_once("=").ok_or_else(|| {
                    DError::grub_parse_error(
                        dctx!(),
                        "Malformed grubenv. Expected '=' after saved_entry",
                    )
                })?;

                let value = split.1.trim();
                if value.is_empty() {
                    return Err(DError::grub_parse_error(
                        dctx!(),
                        "Malformed grubenv. Expected value after saved_entry",
                    ));
                }

                let value = if let Ok(index) = value.parse::<usize>() {
                    Grub2EnvValue::Index(index)
                } else {
                    Grub2EnvValue::Name(value)
                };

                Ok(value)
            });

        let selected = if let Some(value) = selected_idx {
            let value = value?;
            let entry = match value {
                Grub2EnvValue::Index(idx) => entries.get(idx).cloned(),
                Grub2EnvValue::Name(name) => entries
                    .iter()
                    .find(|entry| entry.full_path() == name)
                    .cloned(),
            };

            if entry.is_none() {
                log::warn!("Saved kernel '{value}' was defined as saved_entry but not found in grub. Assuming default kernel.");
            }

            entry
        } else {
            log::debug!("No default kernel entry selected, defaulting to first available kernel");
            None
        };

        Ok(Self { entries, selected })
    }

    pub fn entries(&self) -> &[Grub2BootEntry] {
        &self.entries
    }

    pub fn selected(&self) -> Option<String> {
        self.selected
            .as_ref()
            .map(|selected| selected.entry().into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grub2_bootentries_noselect() {
        let config = read_to_string("test_data/grub.cfg").unwrap();
        let grub_env = read_to_string("test_data/grubenv_empty").unwrap();
        let entries = Grub2BootEntries::from_contents(&config, &grub_env).unwrap();

        assert_eq!(entries.entries().len(), 4);
        assert_eq!(entries.entries()[0].entry, "openSUSE Tumbleweed Minimal");
        assert_eq!(entries.entries()[0].submenus, Vec::<String>::new());
        assert_eq!(
            entries.entries()[1].entry,
            "openSUSE Tumbleweed Minimal, with Linux 6.17.5-1-default"
        );
        assert_eq!(
            entries.entries()[1].submenus,
            vec!["Advanced options for openSUSE Tumbleweed Minimal"]
        );
        assert_eq!(
            entries.entries()[2].entry,
            "openSUSE Tumbleweed Minimal, with Linux 6.17.5-1-default (recovery mode)"
        );
        assert_eq!(
            entries.entries()[2].submenus,
            vec!["Advanced options for openSUSE Tumbleweed Minimal"]
        );
        assert_eq!(entries.entries()[3].entry, "UEFI Firmware Settings");
        assert_eq!(entries.entries()[3].submenus, Vec::<String>::new());
        assert_eq!(entries.selected(), None);
    }
}
