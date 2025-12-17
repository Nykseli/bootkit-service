use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct ConfigArgs {
    /// Use session/user message bus connection instead of system
    #[arg(short, long, default_value_t = false)]
    pub session: bool,

    /// Set log level, overriding BOOTKIT_LOG_LEVEL env variable.
    ///
    /// Possible values: "error", "warn", "info", "debug", "trace", or a number 1-5
    #[arg(short, long)]
    pub log_level: Option<tracing::Level>,

    /// Print pretty logging output that includes colors and timestamps
    #[arg(short, long, default_value_t = false)]
    pub pretty: bool,
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
pub const DATABASE_PATH: &'static str = "/var/lib/bootkit/bootkit.db";
#[cfg(feature = "dev")]
pub const DATABASE_PATH: &'static str = "tmp/bootkit.db";

#[cfg(not(feature = "dev"))]
pub const DEFAULT_LOG_LEVEL: tracing::Level = tracing::Level::INFO;
#[cfg(feature = "dev")]
pub const DEFAULT_LOG_LEVEL: tracing::Level = tracing::Level::DEBUG;
