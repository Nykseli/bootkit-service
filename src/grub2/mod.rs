use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs::read_to_string, path::Path};

use crate::{
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

        for (idx, line) in file.lines().enumerate() {
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

enum GrubEnvValue<'a> {
    /// Index of the bootentry
    Index(usize),
    /// Name of the bootentry
    // Name(String),
    Name(&'a str),
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

pub struct GrubBootEntries {
    entries: Vec<GrubBootEntry>,
    selected: Option<GrubBootEntry>,
}

impl GrubBootEntries {
    pub fn new() -> DResult<Self> {
        log::debug!("Reading kenrnel boot entries from /boot/grub2/grub.cfg");
        let contents = read_to_string("/boot/grub2/grub.cfg")
            .ctx(dctx!(), "Cannot read /boot/grub2/grub.cfg")?;
        let entries = GrubBootEntry::parse_entries(&contents)?;

        log::debug!("Reading default boot entry from /boot/grub2/grubenv");
        let contents = read_to_string("/boot/grub2/grubenv")
            .ctx(dctx!(), "Cannot read /boot/grub2/grubenv")?;

        let selected_idx = contents
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
            match value? {
                GrubEnvValue::Index(idx) => entries.get(idx).cloned(),
                GrubEnvValue::Name(name) => {
                    entries.iter().find(|entry| entry.entry() == name).cloned()
                }
            }
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
