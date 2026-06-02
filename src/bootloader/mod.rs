use std::{
    fs::{exists, read_to_string},
    sync::OnceLock,
};

use crate::{
    dctx,
    errors::{DError, DRes, DResult},
};

pub mod parser;
pub mod systemd_boot;

static BOOTLOADER_TYPE: OnceLock<BootloaderType> = OnceLock::new();

#[derive(Debug, Clone, Copy)]
pub enum BootloaderType {
    Grub,
    SystemdBoot,
}

impl BootloaderType {
    pub fn set_system_type(loader: Self) {
        BOOTLOADER_TYPE.get_or_init(|| loader);
    }

    pub fn system_type() -> Self {
        *BOOTLOADER_TYPE
            .get()
            .expect("BOOTLOADER_TYPE should be initialized once at start of the program")
    }
}

fn parse_sysconfig_bootloader() -> DResult<Option<BootloaderType>> {
    log::debug!("Trying to use /etc/sysconfg/bootloader for detecting bootloader");
    if !exists("/etc/sysconfig/bootloader")
        .ctx(dctx!(), "Cannot access /etc/sysconfig/bootloader")?
    {
        return Ok(None);
    }

    let config = read_to_string("/etc/sysconfig/bootloader")
        .ctx(dctx!(), "Cannot read /etc/sysconfig/bootloader")?;
    for line in config.lines() {
        let line = line.trim();
        if line.starts_with("LOADER_TYPE") {
            let value = line.split("=").nth(1).ctx(
                dctx!(),
                "Malformed /etc/sysconfig/bootloader. Expected '=' after LOADER_TYPE",
            )?;

            let value = value.replace(['\'', '"'], "");
            if value == "grub2" || value == "grub2-efi" {
                return Ok(Some(BootloaderType::Grub));
            } else if value == "systemd-boot" {
                return Ok(Some(BootloaderType::SystemdBoot));
            } else {
                return Err(DError::generic(
                    dctx!(),
                    format!("Unsupported LOADER_TYPE '{value}' in /etc/sysconfig/bootloader"),
                ));
            }
        }
    }

    Err(DError::generic(
        dctx!(),
        "LOADER_TYPE was not found in in /etc/sysconfig/bootloader",
    ))
}

pub fn detect_bootloader() -> DResult<BootloaderType> {
    let sysconfig_boot =
        parse_sysconfig_bootloader().ctx(dctx!(), "Failed to analyze /etc/sysconfig/bootloader")?;

    if let Some(loader) = sysconfig_boot {
        return Ok(loader);
    }

    log::warn!("/etc/sysconfig/bootloader not found. Falling back to less accurate bootloader type detection.");

    if exists("/etc/default/grub").ctx(dctx!(), "Cannot access /etc/default/grub")? {
        log::info!("/etc/default/grub detected, assuming grub as booloader");
        return Ok(BootloaderType::Grub);
    }

    if exists("/boot/efi/loader/loader.conf")
        .ctx(dctx!(), "Cannot access /boot/efi/loader/loader.conf")?
    {
        log::info!("/boot/efi/loader/loader.conf detected, assuming systemd-boot as booloader");
        return Ok(BootloaderType::SystemdBoot);
    }

    Err(DError::generic(dctx!(), "Failed to detect bootloader type"))
}
