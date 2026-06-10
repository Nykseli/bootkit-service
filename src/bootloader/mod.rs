use std::{
    fs::{exists, read_to_string},
    sync::OnceLock,
};

use sqlx::{Pool, Sqlite};

use crate::{
    bootloader::{
        grub2::data_handler::Grub2DataHandler, systemd_boot::data_handler::SystemdDataHandler,
    },
    data::{
        types::{BootkitConfig, BootkitConfigsRaw, BootkitSnapshotSelect, BootkitSnapshots},
        BootkitDataHandler,
    },
    dctx,
    errors::{DError, DRes, DResult},
};

pub mod grub2;
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

/// Data handler enum for static async dispatching until
/// async traits can be dynamically dispatched
#[derive(Clone)]
pub enum BootloaderDataHandler {
    Grub2(Grub2DataHandler),
    SystemdBoot(SystemdDataHandler),
}

impl BootloaderDataHandler {
    pub fn from_loader_type(bootloader: BootloaderType, pool: Pool<Sqlite>) -> Self {
        match bootloader {
            BootloaderType::Grub => Self::Grub2(Grub2DataHandler::new(pool)),
            BootloaderType::SystemdBoot => Self::SystemdBoot(SystemdDataHandler::new(pool)),
        }
    }
}

impl BootkitDataHandler for BootloaderDataHandler {
    async fn get_config(&self) -> DResult<BootkitConfig> {
        match self {
            Self::Grub2(handler) => handler.get_config().await,
            Self::SystemdBoot(handler) => handler.get_config().await,
        }
    }

    async fn save_config(&self, config: &BootkitConfig) -> DResult<()> {
        match self {
            Self::Grub2(handler) => handler.save_config(config).await,
            Self::SystemdBoot(handler) => handler.save_config(config).await,
        }
    }

    async fn get_configs_raw(&self) -> DResult<BootkitConfigsRaw> {
        match self {
            Self::Grub2(handler) => handler.get_configs_raw().await,
            Self::SystemdBoot(handler) => handler.get_configs_raw().await,
        }
    }

    async fn get_snapshots(&self) -> DResult<BootkitSnapshots> {
        match self {
            Self::Grub2(handler) => handler.get_snapshots().await,
            Self::SystemdBoot(handler) => handler.get_snapshots().await,
        }
    }

    async fn select_snapshot(&self, select: &BootkitSnapshotSelect) -> DResult<()> {
        match self {
            Self::Grub2(handler) => handler.select_snapshot(select).await,
            Self::SystemdBoot(handler) => handler.select_snapshot(select).await,
        }
    }

    async fn use_current_snapshot(&self) -> DResult<()> {
        match self {
            Self::Grub2(handler) => handler.use_current_snapshot().await,
            Self::SystemdBoot(handler) => handler.use_current_snapshot().await,
        }
    }

    async fn remove_snapshot(&self, select: &BootkitSnapshotSelect) -> DResult<()> {
        match self {
            Self::Grub2(handler) => handler.remove_snapshot(select).await,
            Self::SystemdBoot(handler) => handler.remove_snapshot(select).await,
        }
    }

    async fn snapshot_from_system(&self) -> DResult<()> {
        match self {
            Self::Grub2(handler) => handler.snapshot_from_system().await,
            Self::SystemdBoot(handler) => handler.snapshot_from_system().await,
        }
    }
}
