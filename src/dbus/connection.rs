use std::{fs::File, io::Write, process::Command};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use zbus::{connection::Builder, interface, object_server::SignalEmitter, Connection, Result};

use crate::{
    config::ConfigArgs,
    grub2::{GrubBootEntries, GrubFile, GrubLine},
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

    async fn save_config(&self, data: &str) -> String {
        // TODO: fail if data is empty
        let config: ConfigData = serde_json::from_str(data).unwrap();
        let value_list: Vec<GrubLine> = serde_json::from_value(config.value_list).unwrap();
        let lines: Vec<String> = value_list.into_iter().map(|val| val.into()).collect();
        let file = lines.join("\n");
        println!("{file}");
        // TODO: start a background thread that executes the grub config
        //       and return an ID that the client can use to poll information

        // WARN: this triggers FileChanged signal
        let mut grub = File::create("/etc/default/grub").unwrap();
        write!(grub, "{}", file).unwrap();

        let mkconfig_child = Command::new("grub2-mkconfig")
            .arg("-o")
            .arg("/boot/grub2/grub.cfg")
            .output()
            .unwrap();

        // TODO: logging
        println!(
            "grub2-mkconfig stdout: {}",
            String::from_utf8(mkconfig_child.stdout).unwrap()
        );
        println!(
            "grub2-mkconfig stderr: {}",
            String::from_utf8(mkconfig_child.stderr).unwrap()
        );

        "ok".to_string()
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
