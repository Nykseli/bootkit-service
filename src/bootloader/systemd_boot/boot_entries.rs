use std::fs::{read, read_dir};

use crate::{
    bootloader::{parser::ConfigFileParser, systemd_boot::loader_config::LoaderConfigFile},
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

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SystemdBootEntry {
    // TODO: since loader config is not a general purpose sturct, should we rename it?
    /// Loader config file. Doesn't exists for autodetected entries
    file: Option<LoaderConfigFile>,
    /// name of the entry file, including .conf
    name: String,
    /// pretty name of the entry, if it exists
    title: Option<String>,
    /// specified kernel arguments if defined
    options: Option<String>,
}

impl SystemdBootEntry {
    fn new(name: &str) -> DResult<Self> {
        if let Some(entry) = Self::auto_detected(name) {
            return Ok(entry);
        }

        let path = format!("{LOADER_ENTRIES_PATH}{name}");
        // TODO: error
        let file = LoaderConfigFile::from_file(&path)
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
