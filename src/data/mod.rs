use crate::{data::types::BootkitConfig, errors::DResult};

pub mod types;

pub trait BootkitDataHandler {
    async fn get_config(&self) -> DResult<BootkitConfig>;
}
