use std::error::Error;

use num::BigUint;
use starknet::providers::jsonrpc::{JsonRpcClient, JsonRpcTransport};
use tokio_util::sync::CancellationToken;
use tracing::info;

use crate::engine::{Engine, Processors};
// use crate::processors::component_register::ComponentRegistrationProcessor;
// use crate::processors::component_state_update::ComponentStateUpdateProcessor;
// use crate::processors::system_register::SystemRegistrationProcessor;
use crate::storage::Storage;

pub async fn start_indexer<S: Storage, T: JsonRpcTransport + Sync + Send>(
    _ct: CancellationToken,
    _world: BigUint,
    storage: &S,
    provider: &JsonRpcClient<T>,
) -> Result<(), Box<dyn Error>> {
    info!("starting indexer");

    let engine = Engine::new(storage, provider, Processors::default());
    engine.start().await?;

    Ok(())
}
