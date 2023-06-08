use std::error::Error;

use dojo_world::manifest::Manifest;
use starknet::providers::jsonrpc::{JsonRpcClient, JsonRpcTransport};
use tracing::info;

use crate::engine::{Engine, EngineConfig, Processors};
use crate::state::State;

#[allow(dead_code)]
pub struct Indexer<'a, S: State, T: JsonRpcTransport + Sync + Send> {
    storage: &'a S,
    provider: &'a JsonRpcClient<T>,
    engine: Engine<'a, S, T>,
    manifest: Manifest,
}

impl<'a, S: State, T: JsonRpcTransport + Sync + Send> Indexer<'a, S, T> {
    pub fn new(
        storage: &'a S,
        provider: &'a JsonRpcClient<T>,
        processors: Processors<S, T>,
        manifest: Manifest,
    ) -> Self {
        let engine = Engine::new(storage, provider, processors, EngineConfig::default());
        Self { storage, provider, engine, manifest }
    }

    pub async fn start(&self) -> Result<(), Box<dyn Error>> {
        info!("starting indexer");
        self.engine.start().await
    }
}
