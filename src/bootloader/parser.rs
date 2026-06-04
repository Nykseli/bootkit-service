use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyValue {
    line: usize,
    original: String,
    changed: bool,

    pub key: String,
    pub value: String,
}

impl KeyValue {
    pub fn new<KV: Into<String>>(line: usize, original: KV, key: KV, value: KV) -> Self {
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
}

#[derive(Debug, Clone)]
pub struct ConfigFile {
    lines: Vec<FileLine>,
}

impl ConfigFile {
    pub fn new(lines: Vec<FileLine>) -> Self {
        Self { lines }
    }
}

pub trait ConfigFileParser {
    fn format_config_line(line: &FileLine) -> String;
    fn config_file(&self) -> &ConfigFile;

    fn lines(&self) -> &[FileLine] {
        &self.config_file().lines
    }

    fn get_key_value(&self, key: &str) -> Option<&KeyValue> {
        self.lines()
            .iter()
            .filter_map(FileLine::key_value)
            .find(|kv| kv.key == key)
    }

    fn as_string(&self) -> String {
        let lines: Vec<String> = self.lines().iter().map(Self::format_config_line).collect();
        lines.join("\n")
    }
}
