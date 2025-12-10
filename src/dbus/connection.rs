use zbus::{connection::Builder, interface, object_server::SignalEmitter, Connection, Result};

use crate::{config::ConfigArgs, db::Database, dbus::handler::DbusHandler};

pub struct BootloaderConfig {
    handler: DbusHandler,
}

#[interface(name = "org.opensuse.bootloader.Config")]
impl BootloaderConfig {
    async fn get_config(&self) -> String {
        log::debug!("Calling org.opensuse.bootloader.Config GetConfig");
        self.handler.get_grub2_config_json().await
    }

    async fn save_config(&self, data: &str) -> String {
        log::debug!("Calling org.opensuse.bootloader.Config SaveConfig");
        self.handler.save_grub2_config(data).await
    }

    /// Signal for grub file being changed, provided by zbus macro
    #[zbus(signal)]
    async fn file_changed(emitter: &SignalEmitter<'_>) -> Result<()>;
}

pub struct BootEntry {
    handler: DbusHandler,
}

#[interface(name = "org.opensuse.bootloader.BootEntry")]
impl BootEntry {
    async fn get_entries(&self) -> String {
        log::debug!("Calling org.opensuse.bootloader.BootEntry GetEntries");
        self.handler.get_grub2_boot_entries().await
    }
}

pub async fn create_connection(args: &ConfigArgs, db: &Database) -> Result<Connection> {
    let handler = DbusHandler::new(db.clone());
    let config = BootloaderConfig {
        handler: handler.clone(),
    };
    let bootentry = BootEntry { handler };

    let (connection, contype) = if args.session {
        (Builder::session()?, "session")
    } else {
        (Builder::system()?, "system")
    };

    let connection = connection
        .name("org.opensuse.bootloader")?
        .serve_at("/org/opensuse/bootloader", config)?
        .serve_at("/org/opensuse/bootloader", bootentry)?
        .build()
        .await?;

    log::info!("Started dbus {contype} connection");

    Ok(connection)
}
