use std::{
    fs::{read, read_dir, read_to_string},
    path::Path,
};

use crate::{
    bootloader::parser::{ConfigFile, ConfigFileParser, FileLine, KeyValue},
    data::types::BootkitConfig,
    dctx,
    errors::{DError, DRes, DResOption, DResult},
};

// TODO: add this to config
const EFI_VARS_PATH: &str = "/sys/firmware/efi/efivars/";
const LOADER_ENTRIES_PATH: &str = "/boot/efi/loader/entries/";

#[derive(Debug)]
#[allow(dead_code)]
struct EfiAttribute {
    /// efi variable attributes
    attrs: u32,
    data: Vec<String>,
}

impl EfiAttribute {
    fn new(data: &[u8]) -> DResult<Self> {
        // TODO: TryFromSliceError for DResult
        let attr: [u8; 4] = (&data[0..4]).try_into().map_err(|_| {
            DError::generic(
                dctx!(),
                "efivar data doesn't have the requred 4 attribute bytes",
            )
        })?;

        let attrs = u32::from_le_bytes(attr);

        let data = &data[4..];
        // efi data is utf16 so decode native endian
        let data_16: Vec<u16> = data
            .chunks(2)
            .map(|chunk| u16::from_ne_bytes([chunk[0], chunk[1]]))
            .collect();
        // TODO: FromUtf16Error for DResult
        let data_str = String::from_utf16(&data_16)
            .map_err(|_| DError::generic(dctx!(), "efivar data is not valid utf16 data"))?;
        // The last string also null terminated so split leaves us with a empty string.
        // Filter it out to be safe
        let data: Vec<String> = data_str
            .split('\0')
            .filter(|data| !data.is_empty())
            .map(str::to_string)
            .collect();

        Ok(Self { data, attrs })
    }
}

fn read_efi_var(name: &str) -> DResult<Option<EfiAttribute>> {
    // TODO: figure out where uuid in the file names come from to make this more accurate
    let files =
        read_dir(EFI_VARS_PATH).ctx(dctx!(), format!("Cannot read {EFI_VARS_PATH} directory"))?;
    for file in files {
        let ent = if let Ok(ent) = file {
            ent
        } else {
            continue;
        };

        let path = ent.path();
        if path.is_file() {
            let file_name = if let Some(filename) = path.file_name() {
                filename
            } else {
                continue;
            };

            let file_name = file_name
                .to_str()
                .ctx(dctx!(), "efivar filepath is not a valid utf8 string")?;
            if file_name.starts_with(name) {
                let data =
                    read(&path).ctx(dctx!(), format!("Cannot read bytes from '{path:?}'"))?;
                let attr = EfiAttribute::new(&data).ctx(
                    dctx!(),
                    format!("Failed to get efi attribute data from '{path:?}'"),
                )?;
                return Ok(Some(attr));
            }
        }
    }

    Ok(None)
}

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

/// systemd-boot boot entry config
/// usually located at /boot/efi/loader/entries/
#[derive(Debug, Clone)]
pub struct EntryConfigFile {
    file: ConfigFile,
    pub path: String,
}

impl EntryConfigFile {
    fn new(file: &str, path: String) -> DResult<Self> {
        let mut lines = Vec::new();

        // use split instead of lines to save the trailing empty new line
        // this doesn't handle \r\n but this is very unlikely to run on
        // windows anyways
        // TODO: refactor this and from_file to the config trait
        //       we just need the parse part and checking when line is valid
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
            path,
            file: ConfigFile::new(lines),
        })
    }

    pub fn from_file<P: AsRef<Path>>(path: P) -> DResult<Self> {
        let file = read_to_string(path.as_ref())
            .ctx(dctx!(), format!("Error reading {:?}", path.as_ref()))?;
        Self::new(&file, path.as_ref().to_str().unwrap().to_string())
    }

    pub fn update_config(&mut self, config: &BootkitConfig) {
        // TODO: should no kernel arguments mean we remove the option?
        if let Some(args) = &config.kernel_arguments {
            self.update_or_insert("options", args);
        }
    }
}

impl ConfigFileParser for EntryConfigFile {
    fn config_file(&self) -> &ConfigFile {
        &self.file
    }

    fn config_file_mut(&mut self) -> &mut ConfigFile {
        &mut self.file
    }

    fn format_key_value(key_value: &KeyValue) -> String {
        format!("{} {}", key_value.key, key_value.value)
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SystemdBootEntry {
    /// Loader config file. Doesn't exists for autodetected entries
    pub file: Option<EntryConfigFile>,
    /// name of the entry file, including .conf
    name: String,
    /// pretty name of the entry, if it exists
    title: Option<String>,
    /// specified kernel arguments if defined
    options: Option<String>,
}

impl SystemdBootEntry {
    pub fn new(name: &str) -> DResult<Self> {
        if let Some(entry) = Self::auto_detected(name) {
            return Ok(entry);
        }

        let path = format!("{LOADER_ENTRIES_PATH}{name}");
        let file = EntryConfigFile::from_file(&path)
            .ctx(dctx!(), format!("Failed to load config from {path}"))?;

        let title = file.get_key_value("title").map(|kv| kv.value.clone());
        let options = file.get_key_value("options").map(|kv| kv.value.clone());

        Ok(Self {
            title,
            options,
            name: name.to_string(),
            file: Some(file),
        })
    }

    /// Systemd-boot has auto detected boot entries, that we can autogenrate
    /// entry information for
    /// See the table in Options -> default: https://www.freedesktop.org/software/systemd/man/latest/loader.conf.html
    fn auto_detected(name: &str) -> Option<Self> {
        let title = if name == "auto-efi-default" {
            "EFI Default Loader"
        } else if name == "auto-efi-shell" {
            "EFI Shell"
        } else if name == "auto-osx" {
            "macOS"
        } else if name == "auto-poweroff" {
            "Power Off The System"
        } else if name == "auto-reboot" {
            "Reboot The System"
        } else if name == "auto-reboot-to-firmware-setup" {
            "Reboot Into Firmware Interface"
        } else if name == "auto-windows" {
            "Windows Boot Manager"
        } else {
            return None;
        };

        Some(Self {
            file: None,
            options: None,
            title: Some(title.to_string()),
            name: name.to_string(),
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    pub fn options(&self) -> Option<&str> {
        self.options.as_deref()
    }
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct SystemdBootEntries {
    pub entries: Vec<SystemdBootEntry>,
    pub selected: Option<SystemdBootEntry>,
}

impl SystemdBootEntries {
    pub fn new() -> DResult<Self> {
        log::debug!("Reading kernel boot entries from {EFI_VARS_PATH}");

        // TODO: also take loader.conf into account if LoaderEntrySelected is not set
        let entries = read_efi_var("LoaderEntries")
            .flat_ctx(dctx!(), "Wasn't able to find LoaderEntries efi variable")?;
        log::trace!("Found systemd-boot LoaderEntries: {entries:#?}");

        let entries: DResult<Vec<SystemdBootEntry>> = entries
            .data
            .iter()
            .map(|name| SystemdBootEntry::new(name))
            .collect();
        let entries = entries.ctx(dctx!(), "Failed to read and parse systemd-boot entries")?;

        let selected = read_efi_var("LoaderEntryDefault")
            .ctx(dctx!(), "Error reading LoaderEntryDefault efivariable")?
            .and_then(|attr| entries.iter().find(|entry| entry.name == attr.data[0]))
            .cloned();

        log::trace!("Found systemd-boot selected entry: {selected:#?}");

        Ok(Self { entries, selected })
    }
}
