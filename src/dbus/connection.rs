use zbus::{connection::Builder, interface, Connection, Result};

use crate::grub2::GrubFile;

pub struct BootloaderConfig {}

#[interface(name = "org.opensuse.bootloader.Config")]
impl BootloaderConfig {
    async fn get_config(&self) -> String {
        let grub = GrubFile::new("/etc/default/grub");
        grub.values()
            .iter()
            .map(|val| format!("{}=\"{}\"", val.key, val.value))
            .collect::<Vec<String>>()
            .join("\n")
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
