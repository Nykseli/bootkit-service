use zbus::{connection::Builder, interface, object_server::SignalEmitter, Connection, Result};

use crate::{config::ConfigArgs, db::Database, dbus::handler::DbusHandler};

struct BootKitInfo {}

#[interface(name = "org.opensuse.bootkit.Info")]
impl BootKitInfo {
    async fn get_version(&self) -> String {
        log::debug!("Calling org.opensuse.bootkit.Info GetVersion");
        env!("CARGO_PKG_VERSION").into()
    }
}

pub struct BootKitSnapshots {
    handler: DbusHandler,
}

#[interface(name = "org.opensuse.bootkit.Snapshot")]
impl BootKitSnapshots {
    async fn get_snapshots(&self) -> String {
        log::debug!("Calling org.opensuse.bootkit.Snapshot GetSnapshots");
        self.handler.get_snapshots().await
    }

    async fn remove_snapshot(&self, data: &str) -> String {
        log::debug!("Calling org.opensuse.bootkit.Snapshot RemoveSnapshot");
        self.handler.remove_snapshot(data).await
    }

    async fn select_snapshot(&self, data: &str) -> String {
        log::debug!("Calling org.opensuse.bootkit.Snapshot SelectSnapshot");
        self.handler.select_snapshot(data).await
    }
}

pub struct BootKitConfig {
    handler: DbusHandler,
}

#[interface(name = "org.opensuse.bootkit.Config")]
impl BootKitConfig {
    async fn get_config(&self) -> String {
        log::debug!("Calling org.opensuse.bootkit.Config GetConfig");
        self.handler.get_grub2_config_json().await
    }

    async fn save_config(&self, data: &str) -> String {
        log::debug!("Calling org.opensuse.bootkit.Config SaveConfig");
        self.handler.save_grub2_config(data).await
    }

    /// Signal for grub file being changed, provided by zbus macro
    #[zbus(signal)]
    async fn file_changed(emitter: &SignalEmitter<'_>) -> Result<()>;
}

pub struct BootEntry {
    handler: DbusHandler,
}

#[interface(name = "org.opensuse.bootkit.BootEntry")]
impl BootEntry {
    async fn get_entries(&self) -> String {
        log::debug!("Calling org.opensuse.bootkit.BootEntry GetEntries");
        self.handler.get_grub2_boot_entries().await
    }
}

pub async fn create_connection(args: &ConfigArgs, db: &Database) -> Result<Connection> {
    let handler = DbusHandler::new(db.clone());
    let config = BootKitConfig {
        handler: handler.clone(),
    };
    let snapshots = BootKitSnapshots {
        handler: handler.clone(),
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
