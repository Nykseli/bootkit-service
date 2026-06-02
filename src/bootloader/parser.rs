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
