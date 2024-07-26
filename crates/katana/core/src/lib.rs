#![cfg_attr(not(test), warn(unused_crate_dependencies))]

use std::sync::Arc;

use constants::MAX_RECURSION_DEPTH;
use katana_executor::implementation::blockifier::BlockifierFactory;
use katana_executor::SimulationFlag;
use katana_primitives::env::{CfgEnv, FeeTokenAddressses};

use crate::backend::config::StarknetConfig;
use crate::backend::Backend;
use crate::pool::TransactionPool;
use crate::service::block_producer::BlockProducer;
#[cfg(feature = "messaging")]
use crate::service::messaging::MessagingService;
use crate::service::{NodeService, TransactionMiner};

pub mod backend;
pub mod constants;
pub mod env;
pub mod pool;
pub mod sequencer;
pub mod service;
pub mod utils;

#[allow(deprecated)]
use sequencer::SequencerConfig;

/// Build the core Katana components from the given configurations.
// TODO: placeholder until we implement a dedicated class that encapsulate building the node
// components
//
// Most of the logic are taken out of the `main.rs` file in `/bin/katana` directory, and combined
// with the exact copy of the setup logic for `NodeService` from `KatanaSequencer::new`.
#[allow(deprecated)]
pub async fn build_node_components(
    config: SequencerConfig,
    starknet_config: StarknetConfig,
) -> anyhow::Result<(
    Arc<TransactionPool>,
    Arc<Backend<BlockifierFactory>>,
    Arc<BlockProducer<BlockifierFactory>>,
)> {
    // build executor factory
    let cfg_env = CfgEnv {
        chain_id: starknet_config.env.chain_id,
        invoke_tx_max_n_steps: starknet_config.env.invoke_max_steps,
        validate_max_n_steps: starknet_config.env.validate_max_steps,
        max_recursion_depth: MAX_RECURSION_DEPTH,
        fee_token_addresses: FeeTokenAddressses {
            eth: starknet_config.genesis.fee_token.address,
            strk: Default::default(),
        },
    };

    let simulation_flags = SimulationFlag {
        skip_validate: starknet_config.disable_validate,
        skip_fee_transfer: starknet_config.disable_fee,
        ..Default::default()
    };

    let executor_factory = Arc::new(BlockifierFactory::new(cfg_env, simulation_flags));
    let backend = Arc::new(Backend::new(executor_factory.clone(), starknet_config).await);

    let pool = Arc::new(TransactionPool::new());
    let miner = TransactionMiner::new(pool.add_listener());

    let block_producer = if config.block_time.is_some() || config.no_mining {
        if let Some(interval) = config.block_time {
            BlockProducer::interval(Arc::clone(&backend), interval)
        } else {
            BlockProducer::on_demand(Arc::clone(&backend))
        }
    } else {
        BlockProducer::instant(Arc::clone(&backend))
    };

    #[cfg(feature = "messaging")]
    let messaging = if let Some(config) = config.messaging.clone() {
        MessagingService::new(config, Arc::clone(&pool), Arc::clone(&backend)).await.ok()
    } else {
        None
    };

    let block_producer = Arc::new(block_producer);

    // TODO: avoid dangling task, or at least store the handle to the NodeService
    tokio::spawn(NodeService::new(
        Arc::clone(&pool),
        miner,
        block_producer.clone(),
        #[cfg(feature = "messaging")]
        messaging,
    ));

    Ok((pool, backend, block_producer))
}
