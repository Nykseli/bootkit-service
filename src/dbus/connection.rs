use zbus::{connection::Builder, fdo, interface, object_server::SignalEmitter, Connection};

use crate::{
    bootloader::{BootloaderDataHandler, BootloaderType},
    config::ConfigArgs,
    data::{
        types::{BootkitConfig, BootkitSnapshotSelect},
        BootkitDataHandler,
    },
    db::{Database, InitializedDb},
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
    handler: BootloaderDataHandler,
}

#[interface(name = "org.opensuse.bootkit.Snapshot")]
impl BootKitSnapshots {
    async fn get_snapshots(&self) -> Result<String, fdo::Error> {
        log::debug!("Calling org.opensuse.bootkit.Snapshot GetSnapshots");

        let data = self
            .handler
            .get_snapshots()
            .await
            .ctx(dctx!(), "Failed to get bootloader snapshots")?
            .serialize()
            .ctx(dctx!(), "Failed to serialise bootloader snapshots")?;

        Ok(data)
    }

    async fn remove_snapshot(&self, data: &str) -> Result<String, fdo::Error> {
        log::debug!("Calling org.opensuse.bootkit.Snapshot RemoveSnapshot");
        let select = BootkitSnapshotSelect::deserialize(data).ctx(
            dctx!(),
            "Failed to parse json data to BootkitSnapshotSelect",
        )?;

        self.handler
            .remove_snapshot(&select)
            .await
            .ctx(dctx!(), "Failed to remove bootloader snapshot")?;

        // TODO: structured response?
        Ok(String::from("ok"))
    }

    async fn select_snapshot(&self, data: &str) -> Result<String, fdo::Error> {
        log::debug!("Calling org.opensuse.bootkit.Snapshot SelectSnapshot");
        let select = BootkitSnapshotSelect::deserialize(data).ctx(
            dctx!(),
            "Failed to parse json data to BootkitSnapshotSelect",
        )?;

        self.handler
            .select_snapshot(&select)
            .await
            .ctx(dctx!(), "Failed to select bootloader snapshot")?;

        // TODO: structured response?
        Ok(String::from("ok"))
    }

    async fn use_current_snapshot(&self) -> Result<String, fdo::Error> {
        log::debug!("Calling org.opensuse.bootkit.Snapshot UseCurrentSnapshot");
        self.handler
            .use_current_snapshot()
            .await
            .ctx(dctx!(), "Failed to use current systemd-boot snapshot")?;

        // TODO: structured response?
        Ok(String::from("ok"))
    }
}

pub struct BootKitConfig {
    handler: BootloaderDataHandler,
}

#[interface(name = "org.opensuse.bootkit.Config")]
impl BootKitConfig {
    async fn get_config(&self) -> Result<String, fdo::Error> {
        log::debug!("Calling org.opensuse.bootkit.Config GetConfig");
        let data = self
            .handler
            .get_config()
            .await
            .ctx(dctx!(), "Failed to get bootloader config")?
            .serialize()
            .ctx(dctx!(), "Failed to serialise bootloader config")?;

        Ok(data)
    }

    async fn save_config(&self, data: &str) -> Result<String, fdo::Error> {
        log::debug!("Calling org.opensuse.bootkit.Config SaveConfig");
        let config = BootkitConfig::deserialize(data)
            .ctx(dctx!(), "Failed to parse json data to BootkitConfig")?;
        self.handler
            .save_config(&config)
            .await
            .ctx(dctx!(), "Failed to get bootloader config")?;

        // TODO: structured response?
        Ok(String::from("ok"))
    }

    /// Signal for grub file being changed, provided by zbus macro
    #[zbus(signal)]
    async fn file_changed(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;
}

pub async fn create_connection(
    args: &ConfigArgs,
    db: Database<InitializedDb>,
) -> zbus::Result<Connection> {
    let handler =
        BootloaderDataHandler::from_loader_type(BootloaderType::system_type(), db.pool().clone());
    let config = BootKitConfig {
        handler: handler.clone(),
    };
    let snapshots = BootKitSnapshots {
        handler: handler.clone(),
    };

    let (connection, contype) = if args.session {
        (Builder::session()?, "session")
    } else {
        (Builder::system()?, "system")
    };

    let connection = connection
        .name("org.opensuse.bootkit")?
        .serve_at("/org/opensuse/bootkit", BootKitInfo {})?
        .serve_at("/org/opensuse/bootkit", config)?
        .serve_at("/org/opensuse/bootkit", snapshots)?
        .build()
        .await?;

    log::info!("Started dbus {contype} connection");

    Ok(connection)
}
