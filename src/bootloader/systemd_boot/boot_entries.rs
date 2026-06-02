use std::fs::{read, read_dir};

use crate::{
    dctx,
    errors::{DError, DRes, DResOption, DResult},
};

// TODO: add this to config
const EFI_VARS_PATH: &str = "/sys/firmware/efi/efivars/";

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

#[derive(Debug)]
#[allow(dead_code)]
pub struct SystemdBootEntry {
    pub entries: Vec<String>,
    pub selected: Option<String>,
}

impl SystemdBootEntry {
    pub fn new() -> DResult<Self> {
        log::debug!("Reading kernel boot entries from {EFI_VARS_PATH}");

        // TODO: also take loader.conf into account if LoaderEntrySelected is not set
        let entries = read_efi_var("LoaderEntries")
            .flat_ctx(dctx!(), "Wasn't able to find LoaderEntries efi variable")?;
        log::trace!("Found systemd-boot LoaderEntries: {entries:#?}");
        let entries = entries.data;

        let selected = read_efi_var("LoaderEntryDefault")
            .ctx(dctx!(), "Error reading LoaderEntryDefault efivariable")?
            .map(|attr| attr.data[0].clone());

        log::trace!("Found systemd-boot selected entry: {selected:#?}");

        Ok(Self { entries, selected })
    }
}
