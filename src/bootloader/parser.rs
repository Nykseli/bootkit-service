use std::{fs::File, io::Write, path::PathBuf};

use serde::{Deserialize, Serialize};
use similar::TextDiff;

use crate::{
    dctx,
    errors::{DRes, DResult},
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
    pub fn new<O, K, V>(line: usize, original: O, key: K, value: V) -> Self
    where
        O: Into<String>,
        K: Into<String>,
        V: Into<String>,
    {
        Self {
            line,
            key: key.into(),
            value: value.into(),
            changed: false,
            original: original.into(),
        }
    }

    pub fn original(&self) -> &str {
        &self.original
    }

    pub fn changed(&self) -> bool {
        self.changed
    }

    pub fn update<V: Into<String>>(&mut self, value: V) {
        let new_value = value.into();
        if self.value != new_value {
            self.changed = true;
            self.value = new_value;
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "t")]
pub enum FileLine {
    KeyValue(KeyValue),
    String { raw_line: String },
}

impl FileLine {
    pub fn key_value(&self) -> Option<&KeyValue> {
        match self {
            FileLine::KeyValue(key_value) => Some(key_value),
            _ => None,
        }
    }

    pub fn key_value_mut(&mut self) -> Option<&mut KeyValue> {
        match self {
            FileLine::KeyValue(key_value) => Some(key_value),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConfigFile {
    path: PathBuf,
    lines: Vec<FileLine>,
}

impl ConfigFile {
    pub fn new<P: Into<PathBuf>>(path: P, lines: Vec<FileLine>) -> Self {
        Self {
            lines,
            path: path.into(),
        }
    }

    fn has_trailing_new_line(&self) -> bool {
        if let Some(FileLine::String { raw_line }) = self.lines.last() {
            // empty line means that the line only had a newline character
            raw_line.is_empty()
        } else {
            false
        }
    }

    pub fn add_key_value_line<K: Into<String>, V: Into<String>>(&mut self, key: K, value: V) {
        // TODO: keep the empty new line end of file IF there was one already
        let mut kv = KeyValue::new(self.lines.len(), String::new(), key, value);
        // a bit of a hack to make sure original value is ignored when formatting etc
        kv.changed = true;

        let line = FileLine::KeyValue(kv);
        if !self.has_trailing_new_line() {
            self.lines.push(line);
        } else {
            // insert line where nl used to be
            self.lines.insert(self.lines.len() - 1, line);
        }
    }

    pub fn get_key_value(&self, key: &str) -> Option<&KeyValue> {
        self.lines
            .iter()
            .filter_map(FileLine::key_value)
            .rfind(|kv| kv.key == key)
    }

    pub fn get_key_value_mut(&mut self, key: &str) -> Option<&mut KeyValue> {
        self.lines
            .iter_mut()
            .filter_map(FileLine::key_value_mut)
            .rfind(|kv| kv.key == key)
    }

    pub fn remove_existing_key_value(&mut self, key: &str) {
        let kv_idx = if let Some((idx, _)) = self
            .lines
            .iter()
            .filter_map(FileLine::key_value)
            .enumerate()
            .find(|(_, kv)| kv.key == key)
        {
            idx
        } else {
            return;
        };

        self.lines.remove(kv_idx);
    }

    /// Ordered list of key -> value strings
    pub fn as_raw_values(&self) -> Vec<(String, String)> {
        self.lines
            .iter()
            .filter_map(FileLine::key_value)
            .map(|kv| (kv.key.clone(), kv.value.clone()))
            .collect()
    }
}

#[allow(dead_code)]
pub trait ConfigFileParser {
    fn format_key_value(key_value: &KeyValue) -> String;
    fn config_file(&self) -> &ConfigFile;
    fn config_file_mut(&mut self) -> &mut ConfigFile;

    fn format_config_line(line: &FileLine) -> String {
        match line {
            FileLine::KeyValue(key_value) => {
                if !key_value.changed() {
                    key_value.original().into()
                } else {
                    Self::format_key_value(key_value)
                }
            }
            FileLine::String { raw_line } => raw_line.into(),
        }
    }

    fn path(&self) -> &PathBuf {
        &self.config_file().path
    }

    fn path_string(&self) -> DResult<String> {
        Ok(self
            .config_file()
            .path
            .to_str()
            .ctx(dctx!(), "File PathBuf is not a valid string")?
            .to_string())
    }

    fn lines(&self) -> &[FileLine] {
        &self.config_file().lines
    }

    fn lines_mut(&mut self) -> &mut [FileLine] {
        &mut self.config_file_mut().lines
    }

    fn get_key_value(&self, key: &str) -> Option<&KeyValue> {
        self.config_file().get_key_value(key)
    }

    fn get_key_value_mut(&mut self, key: &str) -> Option<&mut KeyValue> {
        self.config_file_mut().get_key_value_mut(key)
    }

    fn as_string(&self) -> String {
        let lines: Vec<String> = self.lines().iter().map(Self::format_config_line).collect();
        lines.join("\n")
    }

    fn remove_if_exists<K: AsRef<str>>(&mut self, key: K) {
        let key = key.as_ref();
        self.config_file_mut().remove_existing_key_value(key);
    }

    fn update_if_exists<K: AsRef<str>, V: Into<String>>(&mut self, key: K, value: V) {
        let key = key.as_ref();
        if let Some(key_val) = self.get_key_value_mut(key) {
            key_val.update(value);
        }
    }

    fn update_or_insert<K: AsRef<str>, V: Into<String>>(&mut self, key: K, value: V) {
        let key = key.as_ref();
        if let Some(key_val) = self.get_key_value_mut(key) {
            key_val.update(value);
        } else {
            self.config_file_mut().add_key_value_line(key, value);
        };
    }

    fn save(&self) -> DResult<()> {
        let path = self.path();
        log::debug!("Writing to file {path:?}");

        let mut file = File::create(path).ctx(
            dctx!(),
            format!("Couldn't open file '{path:?}' for reading"),
        )?;

        let content = self.as_string();
        log::trace!("Writen content:\n{content}");
        write!(file, "{}", content).ctx(dctx!(), format!("Failed to write to file '{path:?}'"))
    }

    fn compare_diff_str<O: AsRef<str>>(&self, other: O) -> Option<String> {
        let other = other.as_ref();
        let text_diff = TextDiff::from_lines(self.as_string().as_str(), other)
            .unified_diff()
            .to_string();

        // TextDiff doesn't have a better API for detecting if the files
        // are identical so checking if the contents are empty is our best guess
        if text_diff.trim().is_empty() {
            None
        } else {
            Some(text_diff)
        }
    }

    fn compare_diff(&self, other: &Self) -> Option<String> {
        self.compare_diff_str(other.as_string())
    }
}
