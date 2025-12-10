use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct ConfigArgs {
    /// Use session/user message bus connection instead of system
    #[arg(short, long, default_value_t = false)]
    pub session: bool,
}

#[cfg(not(feature = "dev"))]
pub const GRUB_FILE_PATH: &'static str = "/etc/default/grub";
#[cfg(feature = "dev")]
pub const GRUB_FILE_PATH: &'static str = "tmp/grub";

#[cfg(not(feature = "dev"))]
pub const GRUB_ROOT_PATH: &'static str = "/etc/default";
#[cfg(feature = "dev")]
pub const GRUB_ROOT_PATH: &'static str = "tmp";

#[cfg(not(feature = "dev"))]
pub const DATABASE_PATH: &'static str = "/var/lib/lastlog/lastlog2.db";
#[cfg(feature = "dev")]
pub const DATABASE_PATH: &'static str = "tmp/bootloader.db";

#[cfg(not(feature = "dev"))]
pub const DEFAULT_LOG_LEVEL: tracing::Level = tracing::Level::INFO;
#[cfg(feature = "dev")]
pub const DEFAULT_LOG_LEVEL: tracing::Level = tracing::Level::DEBUG;
