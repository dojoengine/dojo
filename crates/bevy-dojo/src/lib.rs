#[cfg(not(target_arch = "wasm32"))]
pub mod indexer;
pub mod light_client;

pub mod prelude {
    #[cfg(not(target_arch = "wasm32"))]
    pub use crate::indexer::IndexerPlugin;
    pub use crate::light_client::LightClientPlugin;
}
