use crate::{
    data::types::{BootkitConfig, BootkitSnapshots},
    errors::DResult,
};

pub mod types;

pub trait BootkitDataHandler {
    async fn get_config(&self) -> DResult<BootkitConfig>;
    async fn get_snapshots(&self) -> DResult<BootkitSnapshots>;
}
