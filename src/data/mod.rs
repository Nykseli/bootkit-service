use crate::{
    data::types::{BootkitConfig, BootkitConfigsRaw, BootkitSnapshotSelect, BootkitSnapshots},
    errors::DResult,
};

pub mod types;

pub trait BootkitDataHandler {
    async fn get_config(&self) -> DResult<BootkitConfig>;
    async fn save_config(&self, config: &BootkitConfig) -> DResult<()>;
    async fn get_configs_raw(&self) -> DResult<BootkitConfigsRaw>;
    async fn get_snapshots(&self) -> DResult<BootkitSnapshots>;
    async fn select_snapshot(&self, select: &BootkitSnapshotSelect) -> DResult<()>;
    async fn use_current_snapshot(&self) -> DResult<()>;
    /// Create a new snapshot from the state of the system
    async fn snapshot_from_system(&self) -> DResult<()>;
    async fn remove_snapshot(&self, select: &BootkitSnapshotSelect) -> DResult<()>;
}
