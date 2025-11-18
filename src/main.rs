use clap::Parser;
use std::future::pending;
use zbus::Result;

mod config;
mod dbus;
mod events;
mod grub2;
use crate::{config::ConfigArgs, dbus::connection::create_connection, events::listen_files};

#[tokio::main]
async fn main() -> Result<()> {
    let args = ConfigArgs::parse();

    let connection = create_connection(&args).await?;
    listen_files(&connection).await?;
    pending::<()>().await;
    Ok(())
}
