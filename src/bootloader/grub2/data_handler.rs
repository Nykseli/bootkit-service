use crate::{
    data::{
        types::{BootkitConfig, BootkitSnapshotSelect, BootkitSnapshots},
        BootkitDataHandler,
    },
    dctx,
    errors::{DError, DResult},
};

#[derive(Clone)]
pub struct Grub2DataHandler {}

impl BootkitDataHandler for Grub2DataHandler {
    async fn get_config(&self) -> DResult<BootkitConfig> {
        Err(DError::generic(dctx!(), "Not implemented"))
    }

    async fn save_config(&self, _config: &BootkitConfig) -> DResult<()> {
        Err(DError::generic(dctx!(), "Not implemented"))
    }

    async fn get_snapshots(&self) -> DResult<BootkitSnapshots> {
        Err(DError::generic(dctx!(), "Not implemented"))
    }

    async fn select_snapshot(&self, _select: &BootkitSnapshotSelect) -> DResult<()> {
        Err(DError::generic(dctx!(), "Not implemented"))
    }

    async fn use_current_snapshot(&self) -> DResult<()> {
        Err(DError::generic(dctx!(), "Not implemented"))
    }

    async fn remove_snapshot(&self, _select: &BootkitSnapshotSelect) -> DResult<()> {
        Err(DError::generic(dctx!(), "Not implemented"))
    }
}
