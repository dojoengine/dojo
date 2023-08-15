use std::error::Error;

use dojo_world::manifest::Manifest;
use starknet::providers::jsonrpc::{JsonRpcClient, JsonRpcTransport};
use starknet_crypto::FieldElement;
use torii_core::sql::Executable;
use torii_core::State;
use tracing::info;

use crate::engine::{Engine, EngineConfig, Processors};

#[allow(dead_code)]
pub struct Indexer<'a, S: State + Executable, T: JsonRpcTransport + Sync + Send> {
    storage: &'a S,
    provider: &'a JsonRpcClient<T>,
    engine: Engine<'a, S, T>,
    manifest: Manifest,
}

impl<'a, S: State + Executable, T: JsonRpcTransport + Sync + Send> Indexer<'a, S, T> {
    pub fn new(
        storage: &'a S,
        provider: &'a JsonRpcClient<T>,
        processors: Processors<S, T>,
        manifest: Manifest,
        world_address: FieldElement,
        start_block: u64,
    ) -> Self {
        let engine = Engine::new(
            storage,
            provider,
            processors,
            EngineConfig { world_address, start_block, ..Default::default() },
        );
        Self { storage, provider, engine, manifest }
    }

    pub async fn start(&self) -> Result<(), Box<dyn Error>> {
        info!("starting indexer");
        self.engine.start().await
    }
}
