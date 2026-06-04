use zbus::{connection::Builder, fdo, interface, object_server::SignalEmitter, Connection};

use crate::{
    bootloader::{systemd_boot::data_handler::SystemdDataHandler, BootloaderType},
    config::ConfigArgs,
    data::{types::BootkitConfig, BootkitDataHandler},
    db::Database,
    dbus::handler::DbusHandler,
    dctx,
    errors::DRes,
};

struct BootKitInfo {}

#[interface(name = "org.opensuse.bootkit.Info")]
impl BootKitInfo {
    async fn get_version(&self) -> Result<String, fdo::Error> {
        log::debug!("Calling org.opensuse.bootkit.Info GetVersion");
        Ok(env!("CARGO_PKG_VERSION").into())
    }

    /// For keeping the service alive
    async fn ping(&self) -> &'static str {
        log::trace!("Calling org.opensuse.bootkit.Info Ping");
        "pong"
    }
}

pub struct BootKitSnapshots {
    handler: DbusHandler,
    systemd: SystemdDataHandler,
}

#[interface(name = "org.opensuse.bootkit.Snapshot")]
impl BootKitSnapshots {
    async fn get_snapshots(&self) -> Result<String, fdo::Error> {
        log::debug!("Calling org.opensuse.bootkit.Snapshot GetSnapshots");
        let data = match BootloaderType::system_type() {
            BootloaderType::Grub => self.handler.get_snapshots_json().await?,
            BootloaderType::SystemdBoot => {
                let snapshots = self
                    .systemd
                    .get_snapshots()
                    .await
                    .ctx(dctx!(), "Failed to get systemd-boot snapshots")?;
                snapshots
                    .serialize()
                    .ctx(dctx!(), "Failed to to serialize systemd-boot snapshots")?
            }
        };
        Ok(data)
    }

    async fn remove_snapshot(&self, data: &str) -> Result<String, fdo::Error> {
        log::debug!("Calling org.opensuse.bootkit.Snapshot RemoveSnapshot");
        let data = self.handler.remove_snapshot(data).await?;
        Ok(data)
    }

    async fn select_snapshot(&self, data: &str) -> Result<String, fdo::Error> {
        log::debug!("Calling org.opensuse.bootkit.Snapshot SelectSnapshot");
        let data = self.handler.select_snapshot(data).await?;
        Ok(data)
    }

    async fn use_current_snapshot(&self) -> Result<String, fdo::Error> {
        log::debug!("Calling org.opensuse.bootkit.Snapshot UseCurrentSnapshot");
        let data = self.handler.use_current_snapshot().await?;
        Ok(data)
    }
}

pub struct BootKitConfig {
    handler: DbusHandler,
    systemd: SystemdDataHandler,
}

#[interface(name = "org.opensuse.bootkit.Config")]
impl BootKitConfig {
    async fn get_config(&self) -> Result<String, fdo::Error> {
        log::debug!("Calling org.opensuse.bootkit.Config GetConfig");
        let data = match BootloaderType::system_type() {
            BootloaderType::Grub => self.handler.get_grub2_config_json().await?,
            BootloaderType::SystemdBoot => {
                let config = self
                    .systemd
                    .get_config()
                    .await
                    .ctx(dctx!(), "Failed to get systemd-boot config")?;
                config
                    .serialize()
                    .ctx(dctx!(), "Failed to to serialize systemd-boot config")?
            }
        };

        Ok(data)
    }

    async fn save_config(&self, data: &str) -> Result<String, fdo::Error> {
        log::debug!("Calling org.opensuse.bootkit.Config SaveConfig");
        let data = match BootloaderType::system_type() {
            BootloaderType::Grub => self.handler.save_grub2_config(data).await?,
            BootloaderType::SystemdBoot => {
                let config = BootkitConfig::deserialize(data)
                    .ctx(dctx!(), "Failed to parse json data to BootkitConfig")?;
                self.systemd
                    .save_config(&config)
                    .await
                    .ctx(dctx!(), "Failed to get systemd-boot config")?;

                // TODO: structured response?
                String::from("ok")
            }
        };

        Ok(data)
    }

    /// Signal for grub file being changed, provided by zbus macro
    #[zbus(signal)]
    async fn file_changed(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;
}

pub struct BootEntry {
    handler: DbusHandler,
}

#[interface(name = "org.opensuse.bootkit.BootEntry")]
impl BootEntry {
    async fn get_entries(&self) -> Result<String, fdo::Error> {
        log::debug!("Calling org.opensuse.bootkit.BootEntry GetEntries");
        let data = self.handler.get_grub2_boot_entries_json().await?;
        Ok(data)
    }
}

pub async fn create_connection(args: &ConfigArgs, db: &Database) -> zbus::Result<Connection> {
    let handler = DbusHandler::new(db.clone());
    let config = BootKitConfig {
        handler: handler.clone(),
        systemd: SystemdDataHandler::new(db.pool().clone()),
    };
    let snapshots = BootKitSnapshots {
        handler: handler.clone(),
        systemd: SystemdDataHandler::new(db.pool().clone()),
    };
    let bootentry = BootEntry { handler };

    let (connection, contype) = if args.session {
        (Builder::session()?, "session")
    } else {
        (Builder::system()?, "system")
    };

    let connection = connection
        .name("org.opensuse.bootkit")?
        .serve_at("/org/opensuse/bootkit", BootKitInfo {})?
        .serve_at("/org/opensuse/bootkit", config)?
        .serve_at("/org/opensuse/bootkit", bootentry)?
        .serve_at("/org/opensuse/bootkit", snapshots)?
        .build()
        .await?;

    log::info!("Started dbus {contype} connection");

    Ok(connection)
}
