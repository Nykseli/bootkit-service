use crate::{
    data::types::{BootkitConfig, BootkitSnapshotSelect, BootkitSnapshots},
    errors::DResult,
};

pub mod types;

pub trait BootkitDataHandler {
    async fn get_config(&self) -> DResult<BootkitConfig>;
    async fn save_config(&self, config: &BootkitConfig) -> DResult<()>;
    async fn get_snapshots(&self) -> DResult<BootkitSnapshots>;
    async fn select_snapshot(&self, select: &BootkitSnapshotSelect) -> DResult<()>;
}
