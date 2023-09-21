use std::error::Error;

use dojo_world::manifest::Manifest;
use starknet::providers::Provider;
use starknet_crypto::FieldElement;
use torii_client::contract::world::WorldContractReader;
use torii_core::sql::Sql;
use tracing::info;

use crate::engine::{Engine, EngineConfig, Processors};

#[allow(dead_code)]
pub struct Indexer<'a, P: Provider + Sync + Send> {
    world: &'a WorldContractReader<'a, P>,
    db: &'a Sql,
    provider: &'a P,
    engine: Engine<'a, P>,
    manifest: Manifest,
}

impl<'a, P: Provider + Sync + Send> Indexer<'a, P> {
    pub fn new(
        world: &'a WorldContractReader<'a, P>,
        db: &'a Sql,
        provider: &'a P,
        processors: Processors<P>,
        manifest: Manifest,
        world_address: FieldElement,
        start_block: u64,
    ) -> Self {
        let engine = Engine::new(
            world,
            db,
            provider,
            processors,
            EngineConfig { world_address, start_block, ..Default::default() },
        );
        Self { world, db, provider, engine, manifest }
    }

    pub async fn start(&self) -> Result<(), Box<dyn Error>> {
        info!("starting indexer");
        self.engine.start().await
    }
}
