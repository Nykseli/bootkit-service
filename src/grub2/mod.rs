use std::{collections::HashMap, fs::read_to_string, path::Path};

#[derive(Debug, Clone)]
pub struct KeyValue {
    line: usize,
    original: String,
    changed: bool,

    pub key: String,
    pub value: String,
}

impl KeyValue {
    fn new(line: usize, original: &str) -> Self {
        let mut kv = Self {
            line,
            changed: false,
            key: "".into(),
            value: "".into(),
            original: original.into(),
        };

        kv.parse();
        kv
    }

    fn parse(&mut self) {
        // assuming this is always valid
        // TODO: error out if the parse fails
        // TODO: save the type of quotes so they can be returned to orignal
        let trimmed = self.original.trim();
        let split = trimmed.split_once('=').unwrap();
        self.key = split.0.into();
        self.value = split.1.replace('\'', "").replace('"', "").into();
    }
}

#[derive(Debug)]
enum GrubLine {
    KeyValue(KeyValue),
    String(String),
}

impl GrubLine {
    fn as_keyval(&self) -> Option<&KeyValue> {
        match self {
            GrubLine::KeyValue(keyval) => Some(keyval),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub struct GrubFile {
    lines: Vec<GrubLine>,
    keyvals: HashMap<String, KeyValue>,
}

impl GrubFile {
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        let file = read_to_string(path).unwrap();

        let mut lines = Vec::new();
        let mut keyvals = HashMap::new();

        for (idx, line) in file.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                lines.push(GrubLine::String(line.into()));
                continue;
            }

            let keyval = KeyValue::new(idx, line);
            keyvals.insert(keyval.key.clone(), keyval.clone());
            lines.push(GrubLine::KeyValue(keyval));
        }

        Self { lines, keyvals }
    }

    pub fn values(&self) -> Vec<&KeyValue> {
        self.lines.iter().filter_map(|kv| kv.as_keyval()).collect()
    }

    pub fn keyvalues(&self) -> &HashMap<String, KeyValue> {
        &self.keyvals
    }
}
