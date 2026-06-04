use std::{fs::read_to_string, path::Path};

use crate::{
    bootloader::parser::{ConfigFile, ConfigFileParser, FileLine, KeyValue},
    dctx,
    errors::{DError, DRes, DResult},
};

fn parse_config_line(line_num: usize, line: &str) -> DResult<KeyValue> {
    let trimmed = line.trim();
    let mut split = trimmed.split_whitespace();
    let key = split.next().ctx(
        dctx!(),
        format!("Expected whitespace separator on line {}", line_num + 1),
    )?;

    let value = if let Some(value) = trimmed.strip_prefix(key) {
        value.trim()
    } else {
        ""
    };

    if value.is_empty() {
        return Err(DError::generic(
            dctx!(),
            format!("Expected value on line: {}", line_num + 1),
        ));
    }

    Ok(KeyValue::new(line_num, line, key, value))
}

/// systemd-boot loader config file
/// usually located at /boot/efi/loader/loader.conf
#[derive(Debug, Clone)]
pub struct LoaderConfigFile {
    file: ConfigFile,
}

impl LoaderConfigFile {
    pub fn new(file: &str) -> DResult<Self> {
        let mut lines = Vec::new();

        // use split instead of lines to save the trailing empty new line
        // this doesn't handle \r\n but this is very unlikely to run on
        // windows anyways
        for (idx, line) in file.split('\n').enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                lines.push(FileLine::String {
                    raw_line: line.into(),
                });
                continue;
            }

            let keyval = parse_config_line(idx, line)
                .ctx(dctx!(), "Failed to parse systemd-boot config file")?;
            lines.push(FileLine::KeyValue(keyval));
        }

        Ok(Self {
            file: ConfigFile::new(lines),
        })
    }

    pub fn from_file<P: AsRef<Path>>(path: P) -> DResult<Self> {
        let file = read_to_string(path.as_ref())
            .ctx(dctx!(), format!("Error reading {:?}", path.as_ref()))?;
        Self::new(&file)
    }
}

impl ConfigFileParser for LoaderConfigFile {
    fn format_config_line(line: &FileLine) -> String {
        match line {
            FileLine::KeyValue(key_value) => {
                if key_value.changed() {
                    key_value.original().into()
                } else {
                    format!("{} {}", key_value.key, key_value.value)
                }
            }
            FileLine::String { raw_line } => raw_line.into(),
        }
    }

    fn config_file(&self) -> &ConfigFile {
        &self.file
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    impl PartialEq<(&str, &str)> for FileLine {
        fn eq(&self, other: &(&str, &str)) -> bool {
            match self {
                FileLine::KeyValue(key_value) => {
                    key_value.key == other.0 && key_value.value == other.1
                }
                _ => false,
            }
        }
    }

    impl PartialEq<&str> for FileLine {
        fn eq(&self, other: &&str) -> bool {
            match self {
                FileLine::String { raw_line } => raw_line == other,
                _ => false,
            }
        }
    }

    #[test]
    fn test_systemd_config_parsing_no_eol() {
        let file = LoaderConfigFile::new("timeout 10").unwrap();
        let lines = file.lines();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0], ("timeout", "10"));
    }

    #[test]
    fn test_systemd_config_parsing_with_eol() {
        let file = LoaderConfigFile::new("timeout 10\n").unwrap();
        let lines = file.lines();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], ("timeout", "10"));
        // make sure the last line is empty (empty trailing line)
        assert_eq!(lines[1], "");
    }

    #[test]
    fn test_systemd_config_parsing_fail() {
        let err = LoaderConfigFile::new("timeout").unwrap_err();
        assert_eq!(err.error().as_string(), "Error: Expected value on line: 1");
    }

    #[test]
    fn test_systemd_entry_spaces_in_values() {
        let file = LoaderConfigFile::new("options    splash=silent mitigations=auto").unwrap();
        let lines = file.lines();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0], ("options", "splash=silent mitigations=auto"));
    }
}
