use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt::Display, fs::read_to_string, path::Path};

use crate::{
    config::{GRUB_CFG_PATH, GRUB_ENV_PATH},
    dctx,
    errors::{DError, DRes, DResult},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyValue {
    line: usize,
    original: String,
    changed: bool,

    pub key: String,
    pub value: String,
}

impl KeyValue {
    fn new(line: usize, original: &str) -> DResult<Self> {
        let mut kv = Self {
            line,
            changed: false,
            key: "".into(),
            value: "".into(),
            original: original.into(),
        };

        kv.parse()?;
        Ok(kv)
    }

    fn from_key_val<KV: Into<String>>(line: usize, key: KV, value: KV) -> Self {
        Self {
            line,
            original: String::new(),
            changed: true,
            key: key.into(),
            value: value.into(),
        }
    }

    fn parse(&mut self) -> DResult<()> {
        // TODO: save the type of quotes so they can be returned to orignal
        let trimmed = self.original.trim();
        let split = if let Some(split) = trimmed.split_once('=') {
            split
        } else {
            return Err(DError::grub_parse_error(
                dctx!(),
                format!("Expected '=' on line: {}", self.line + 1),
            ));
        };
        self.key = split.0.into();
        self.value = split.1.replace(['\'', '"'], "");

        Ok(())
    }

    fn update<V: Into<String>>(&mut self, value: V) {
        let new_value = value.into();
        if self.value != new_value {
            self.changed = true;
            self.value = new_value;
        }
    }
}

impl From<KeyValue> for String {
    fn from(value: KeyValue) -> Self {
        if !value.changed {
            value.original
        } else {
            format!("{}=\"{}\"", value.key, value.value)
        }
    }
}

impl From<&KeyValue> for String {
    fn from(value: &KeyValue) -> Self {
        if !value.changed {
            value.original.clone()
        } else {
            format!("{}=\"{}\"", value.key, value.value)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "t")]
pub enum GrubLine {
    KeyValue(KeyValue),
    String { raw_line: String },
}

impl From<GrubLine> for String {
    fn from(value: GrubLine) -> Self {
        match value {
            GrubLine::KeyValue(key_value) => key_value.into(),
            GrubLine::String { raw_line } => raw_line,
        }
    }
}

impl From<&GrubLine> for String {
    fn from(value: &GrubLine) -> Self {
        match value {
            GrubLine::KeyValue(key_value) => key_value.into(),
            GrubLine::String { raw_line } => raw_line.into(),
        }
    }
}

#[derive(Debug)]
pub struct GrubFile {
    lines: Vec<GrubLine>,
    keyvals: HashMap<String, KeyValue>,
}

impl GrubFile {
    pub fn new(file: &str) -> DResult<Self> {
        let mut lines = Vec::new();
        let mut keyvals = HashMap::new();

        // use split instead of lines to save the trailing empty new line
        // this doesn't handle \r\n but this is very unlikely to run on
        // windows anyways
        for (idx, line) in file.split('\n').enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                lines.push(GrubLine::String {
                    raw_line: line.into(),
                });
                continue;
            }

            let keyval = KeyValue::new(idx, line)?;
            keyvals.insert(keyval.key.clone(), keyval.clone());
            lines.push(GrubLine::KeyValue(keyval));
        }

        Ok(Self { lines, keyvals })
    }

    pub fn set_key_value(&mut self, key: &str, value: &str) {
        if let Some(keyval) = self.keyvals.get_mut(key) {
            // If keyvalue exists, update it
            keyval.update(value);
            if let GrubLine::KeyValue(keyval) = &mut self.lines[keyval.line] {
                keyval.update(value);
            }
        } else {
            // else add a new value
            let keyval = KeyValue::from_key_val(self.lines.len(), key, value);
            self.keyvals.insert(keyval.key.clone(), keyval.clone());
            self.lines.push(GrubLine::KeyValue(keyval));
        }
    }

    pub fn from_file<P: AsRef<Path>>(path: P) -> DResult<Self> {
        let file = read_to_string(path.as_ref())
            .ctx(dctx!(), format!("Error reading {:?}", path.as_ref()))?;
        Self::new(&file)
    }

    pub fn from_lines(grub_lines: &[GrubLine]) -> Self {
        let mut lines = Vec::new();
        let mut keyvals = HashMap::new();

        for line in grub_lines {
            lines.push(line.clone());
            if let GrubLine::KeyValue(keyval) = line {
                keyvals.insert(keyval.key.clone(), keyval.clone());
            }
        }

        Self { lines, keyvals }
    }

    pub fn lines(&self) -> &[GrubLine] {
        &self.lines
    }

    pub fn keyvalues(&self) -> &HashMap<String, KeyValue> {
        &self.keyvals
    }

    pub fn as_string(&self) -> String {
        let lines: Vec<String> = self.lines().iter().map(|val| val.into()).collect();
        lines.join("\n")
    }
}

#[derive(Debug)]
enum GrubEnvValue<'a> {
    /// Index of the bootentry
    Index(usize),
    /// Name of the bootentry
    // Name(String),
    Name(&'a str),
}

impl Display for GrubEnvValue<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GrubEnvValue::Index(idx) => write!(f, "{idx}"),
            GrubEnvValue::Name(name) => write!(f, "{name}"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct GrubBootEntry {
    /// The actual name of the entry
    entry: String,
    /// (nested) submenus
    submenus: Vec<String>,
}

impl GrubBootEntry {
    fn new(entry: String, submenus: Vec<String>) -> Self {
        Self { entry, submenus }
    }

    fn parse_entries(contents: &str) -> DResult<Vec<GrubBootEntry>> {
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
pub struct GrubBootEntries {
    entries: Vec<GrubBootEntry>,
    selected: Option<GrubBootEntry>,
}

impl GrubBootEntries {
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
        let entries = GrubBootEntry::parse_entries(grub_config)?;

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
                    GrubEnvValue::Index(index)
                } else {
                    GrubEnvValue::Name(value)
                };

                Ok(value)
            });

        let selected = if let Some(value) = selected_idx {
            let value = value?;
            let entry = match value {
                GrubEnvValue::Index(idx) => entries.get(idx).cloned(),
                GrubEnvValue::Name(name) => entries
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

    pub fn entry_names(&self) -> Vec<&str> {
        self.entries.iter().map(|entry| entry.entry()).collect()
    }

    pub fn entries(&self) -> &[GrubBootEntry] {
        // self.entries.iter().map(|entry| entry.entry()).collect()
        &self.entries
    }

    pub fn selected(&self) -> Option<&str> {
        if let Some(selected) = &self.selected {
            Some(selected.entry())
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    impl PartialEq<(&str, &str)> for GrubLine {
        fn eq(&self, other: &(&str, &str)) -> bool {
            match self {
                GrubLine::KeyValue(key_value) => {
                    key_value.key == other.0 && key_value.value == other.1
                }
                _ => false, // GrubLine::String { raw_line } => todo!(),
            }
        }
    }

    impl PartialEq<&str> for GrubLine {
        fn eq(&self, other: &&str) -> bool {
            match self {
                GrubLine::String { raw_line } => raw_line == other,
                _ => false,
            }
        }
    }

    #[test]
    fn test_grub2_parsing_no_eol() {
        let file = GrubFile::new("GRUB_DEFAULT=saved").unwrap();
        let lines = file.lines();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0], ("GRUB_DEFAULT", "saved"));
    }

    #[test]
    fn test_grub2_parsing_with_eol() {
        let file = GrubFile::new("GRUB_DEFAULT=saved\n").unwrap();
        let lines = file.lines();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], ("GRUB_DEFAULT", "saved"));
        // make sure the last line is empty (empty trailing line)
        assert_eq!(lines[1], "");
    }

    #[test]
    fn test_grub2_parsing_fail() {
        let err = GrubFile::new("GRUB_DEFAULT").unwrap_err();
        assert_eq!(
            err.error().as_string(),
            "Internal Parse: Failed to parse grub config: Expected '=' on line: 1"
        );
    }

    #[test]
    fn test_grub2_parsing_simple() {
        let file_data = read_to_string("test_data/grub_simple").unwrap();
        let file = GrubFile::new(&file_data).unwrap();
        let lines = file.lines();
        assert_eq!(lines.len(), 5);
        assert_eq!(lines[0], ("GRUB_DISTRIBUTOR", ""));
        assert_eq!(lines[1], ("GRUB_DEFAULT", "saved"));
        assert_eq!(lines[2], ("GRUB_HIDDEN_TIMEOUT_QUIET", "true"));
        assert_eq!(lines[3], ("GRUB_TIMEOUT", "8"));
        // make sure the last line is empty (empty trailing line)
        assert_eq!(lines[4], "");
        assert_eq!(file.as_string(), file_data);
    }

    #[test]
    fn test_grub2_parsing_full() {
        let file_data = read_to_string("test_data/grub_full").unwrap();
        let file = GrubFile::new(&file_data).unwrap();
        let lines = file.lines();
        assert_eq!(lines.len(), 46);
        assert_eq!(lines[0], "# If you change this file, run \'grub2-mkconfig -o /boot/grub2/grub.cfg\' afterwards to update");
        assert_eq!(lines[1], "# /boot/grub2/grub.cfg.");
        assert_eq!(lines[2], "");
        assert_eq!(lines[3], "# Uncomment to set your own custom distributor. If you leave it unset or empty, the default");
        assert_eq!(
            lines[4],
            "# policy is to determine the value from /etc/os-release"
        );
        assert_eq!(lines[5], ("GRUB_DISTRIBUTOR", ""));
        assert_eq!(lines[6], ("GRUB_DEFAULT", "saved"));
        assert_eq!(lines[7], ("GRUB_HIDDEN_TIMEOUT", "0"));
        assert_eq!(lines[8], ("GRUB_HIDDEN_TIMEOUT_QUIET", "true"));
        assert_eq!(lines[9], ("GRUB_TIMEOUT", "8"));
        assert_eq!(
            lines[10],
            (
                "GRUB_CMDLINE_LINUX_DEFAULT",
                "splash=silent quiet security=apparmor amd_pstate=active mitigations=auto"
            )
        );
        assert_eq!(lines[11], ("GRUB_CMDLINE_LINUX", ""));
        assert_eq!(lines[12], "");
        assert_eq!(
            lines[13],
            "# Uncomment to automatically save last booted menu entry in GRUB2 environment"
        );
        assert_eq!(lines[14], "");
        assert_eq!(lines[15], "# variable `saved_entry\'");
        assert_eq!(lines[16], "# GRUB_SAVEDEFAULT=\"true\"");
        assert_eq!(
            lines[17],
            "#Uncomment to enable BadRAM filtering, modify to suit your needs"
        );
        assert_eq!(lines[18], "");
        assert_eq!(
            lines[19],
            "# This works with Linux (no patch required) and with any kernel that obtains"
        );
        assert_eq!(
            lines[20],
            "# the memory map information from GRUB (GNU Mach, kernel of FreeBSD ...)"
        );
        assert_eq!(
            lines[21],
            "# GRUB_BADRAM=\"0x01234567,0xfefefefe,0x89abcdef,0xefefefef\""
        );
        assert_eq!(
            lines[22],
            "#Uncomment to disable graphical terminal (grub-pc only)"
        );
        assert_eq!(lines[23], "");
        assert_eq!(lines[24], ("GRUB_TERMINAL", "gfxterm"));
        assert_eq!(lines[25], "# The resolution used on graphical terminal");
        assert_eq!(
            lines[26],
            "#note that you can use only modes which your graphic card supports via VBE"
        );
        assert_eq!(lines[27], "");
        assert_eq!(
            lines[28],
            "# you can see them in real GRUB with the command `vbeinfo\'"
        );
        assert_eq!(lines[29], ("GRUB_GFXMODE", "auto"));
        assert_eq!(
            lines[30],
            "# Uncomment if you don\'t want GRUB to pass \"root=UUID=xxx\" parameter to Linux"
        );
        assert_eq!(lines[31], "# GRUB_DISABLE_LINUX_UUID=true");
        assert_eq!(
            lines[32],
            "#Uncomment to disable generation of recovery mode menu entries"
        );
        assert_eq!(lines[33], "");
        assert_eq!(lines[34], "# GRUB_DISABLE_RECOVERY=\"true\"");
        assert_eq!(lines[35], "#Uncomment to get a beep at grub start");
        assert_eq!(lines[36], "");
        assert_eq!(lines[37], "# GRUB_INIT_TUNE=\"480 440 1\"");
        assert_eq!(lines[38], ("GRUB_BACKGROUND", ""));
        assert_eq!(
            lines[39],
            ("GRUB_THEME", "/boot/grub2/themes/openSUSE/theme.txt")
        );
        assert_eq!(lines[40], ("SUSE_BTRFS_SNAPSHOT_BOOTING", "true"));
        assert_eq!(lines[41], ("GRUB_USE_LINUXEFI", "true"));
        assert_eq!(lines[42], ("GRUB_DISABLE_OS_PROBER", "false"));
        assert_eq!(lines[43], ("GRUB_ENABLE_CRYPTODISK", "y"));
        assert_eq!(
            lines[44],
            ("GRUB_CMDLINE_XEN_DEFAULT", "vga=gfx-1024x768x16")
        );
        assert_eq!(lines[45], "");

        assert_eq!(file.as_string(), file_data);
    }

    #[test]
    fn test_grub2_bootentries_noselect() {
        let config = read_to_string("test_data/grub.cfg").unwrap();
        let grub_env = read_to_string("test_data/grubenv_empty").unwrap();
        let entries = GrubBootEntries::from_contents(&config, &grub_env).unwrap();

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
