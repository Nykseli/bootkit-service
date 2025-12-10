use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs::read_to_string, io, path::Path};

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

    fn parse(&mut self) -> DResult<()> {
        // TODO: save the type of quotes so they can be returned to orignal
        let trimmed = self.original.trim();
        let split = if let Some(split) = trimmed.split_once('=') {
            split
        } else {
            return DError::grub_parse_error(
                dctx!(),
                format!("Expected '=' on line: {}", self.line + 1),
            );
        };
        self.key = split.0.into();
        self.value = split.1.replace('\'', "").replace('"', "").into();

        Ok(())
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
        let lines: Vec<String> = self.lines().into_iter().map(|val| val.into()).collect();
        lines.join("\n")
    }
}

pub struct GrubBootEntries {
    entries: Vec<String>,
}

impl GrubBootEntries {
    pub fn new() -> io::Result<Self> {
        log::debug!("Reading kenrnel boot entries from /boot/grub2/grub.cfg");
        let contents = read_to_string("/boot/grub2/grub.cfg")?;
        // this is unrecovable error so panic is appropriate
        let re = Regex::new(r"menuentry\s+'([^']+)").expect("Invalid regex");
        let entries: Vec<String> = re
            .captures_iter(&contents)
            .map(|capture| capture[1].to_string())
            .collect();
        Ok(Self { entries })
    }

    pub fn entries(&self) -> &[String] {
        &self.entries
    }
}
