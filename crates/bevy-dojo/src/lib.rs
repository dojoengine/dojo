mod light_client;

#[cfg(not(target_arch = "wasm32"))]
mod indexer;

#[cfg(not(target_arch = "wasm32"))]
pub mod apibara {
    pub use {apibara_core as core, apibara_sdk as sdk};

    pub use crate::indexer::*;
}

pub use light_client::*;
