use serde::{Deserialize, Serialize};
use serde_json::Value;
use zbus::{connection::Builder, interface, object_server::SignalEmitter, Connection, Result};

use crate::{
    config::ConfigArgs,
    grub2::{GrubBootEntries, GrubFile},
};

pub struct BootloaderConfig {}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ConfigData {
    value_map: Value,
    value_list: Value,
}

#[interface(name = "org.opensuse.bootloader.Config")]
impl BootloaderConfig {
    async fn get_config(&self) -> String {
        let grub = GrubFile::new("/etc/default/grub");

        let value_map = serde_json::to_value(grub.keyvalues()).unwrap();
        let value_list = serde_json::to_value(grub.lines()).unwrap();
        let data = ConfigData {
            value_list,
            value_map,
        };

        serde_json::to_string(&data).unwrap()
    }

    /// Signal for grub file being changed, provided by zbus macro
    #[zbus(signal)]
    async fn file_changed(emitter: &SignalEmitter<'_>) -> Result<()>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BootEntryData {
    entries: Value,
}

pub struct BootEntry {}

#[interface(name = "org.opensuse.bootloader.BootEntry")]
impl BootEntry {
    async fn get_entries(&self) -> String {
        // TODO: return error
        let grub_entries = GrubBootEntries::new().unwrap();
        let entries = serde_json::to_value(grub_entries.entries()).unwrap();
        let data = BootEntryData { entries };

        serde_json::to_string(&data).unwrap()
    }
}

pub async fn create_connection(args: &ConfigArgs) -> Result<Connection> {
    let config = BootloaderConfig {};
    let bootentry = BootEntry {};

    let connection = if args.session {
        Builder::session()?
    } else {
        Builder::system()?
    };

    let connection = connection
        .name("org.opensuse.bootloader")?
        .serve_at("/org/opensuse/bootloader", config)?
        .serve_at("/org/opensuse/bootloader", bootentry)?
        .build()
        .await?;

    Ok(connection)
}
