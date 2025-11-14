use serde::{Deserialize, Serialize};
use serde_json::Value;
use zbus::{connection::Builder, interface, Connection, Result};

use crate::grub2::GrubFile;

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
        let value_list = serde_json::to_value(grub.values()).unwrap();
        let data = ConfigData {
            value_list,
            value_map,
        };

        serde_json::to_string(&data).unwrap()
    }
}

pub async fn create_connection() -> Result<Connection> {
    let config = BootloaderConfig {};

    let connection = Builder::session()?
        .name("org.opensuse.bootloader.Config")?
        .serve_at("/org/opensuse/bootloader", config)?
        .build()
        .await?;

    Ok(connection)
}
