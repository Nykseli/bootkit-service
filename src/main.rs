use clap::Parser;
use std::future::pending;

mod config;
mod db;
mod dbus;
mod errors;
mod events;
mod grub2;
mod logging;

use crate::{
    config::ConfigArgs,
    db::Database,
    dbus::connection::create_connection,
    errors::{DRes, DResult},
    events::listen_files,
    logging::setup_logging,
};

#[tokio::main]
async fn main() -> DResult<()> {
    let args = ConfigArgs::parse();

    setup_logging();
    log::info!("Starting bootkit service");

    let db = Database::new().await?;
    db.initialize().await?;

    let connection = create_connection(&args, &db)
        .await
        .ctx(dctx!(), "Failed to create Zbus connection")?;
    listen_files(&connection)
        .await
        .ctx(dctx!(), "Failed to listen file events")?;
    pending::<()>().await;
    Ok(())
}
