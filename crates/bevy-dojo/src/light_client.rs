//! Beerus based Ethereum/Starknet light client module.
//!
//! More on [Beerus GitHub page](https://github.com/keep-starknet-strange/beerus)
//!
//! ### Usage
//!
//! ```rust, no_run
//! use bevy::prelude::*;
//! use bevy_dojo::prelude::*;
//!
//! fn main() {
//!     App::new()
//!         .add_plugins(DefaultPlugins)
//!         // Set up Starknet light client plugin.
//!         .add_plugin(LightClientPlugin)
//!         .run();
//! }
//! ```

pub mod ethereum;
pub mod starknet;

pub mod prelude {
    pub use ethereum::*;

    pub use self::starknet::*;
    pub use crate::light_client::*;
}

use beerus_core::config::Config;
use beerus_core::lightclient::beerus::BeerusLightClient;
use beerus_core::lightclient::ethereum::helios_lightclient::HeliosLightClient;
use beerus_core::lightclient::starknet::StarkNetLightClientImpl;
use bevy::app::Plugin;
use bevy::ecs::component::Component;
use bevy::ecs::system::{In, ResMut};
use bevy::log;
use bevy::prelude::IntoPipeSystem;
use bevy_tokio_tasks::TokioTasksRuntime;
use eyre::{Error, Result};
use tokio::sync::mpsc::error::TrySendError;
use tokio::sync::mpsc::Sender;

use self::ethereum::{EthRequest, EthereumClientPlugin};
use self::starknet::{StarknetClientPlugin, StarknetRequest};

/// Plugin to manage Ethereum/Starknet light client.
pub struct LightClientPlugin;

impl Plugin for LightClientPlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        app.add_plugin(EthereumClientPlugin)
            .add_plugin(StarknetClientPlugin)
            .add_startup_system(start_light_client.pipe(handle_light_client_error));
    }
}

fn start_light_client(runtime: ResMut<'_, TokioTasksRuntime>) -> Result<()> {
    log::info!("Starting...");
    let config = Config::from_env();
    let (tx, mut rx) = tokio::sync::mpsc::channel::<LightClientRequest>(1);

    runtime.spawn_background_task(|mut ctx| async move {
        log::info!("Creating ethereum(helios) lightclient...");
        let ethereum_light_client = HeliosLightClient::new(config.clone()).await?;

        log::info!("Creating starknet light client...");
        let starknet_light_client = StarkNetLightClientImpl::new(&config)?;

        log::info!("Creating beerus light client...");
        let mut client = BeerusLightClient::new(
            config,
            Box::new(ethereum_light_client),
            Box::new(starknet_light_client),
        );

        client.start().await?;

        log::info!("Light client is ready");

        ctx.run_on_main_thread(move |ctx| {
            ctx.world.spawn(LightClient::new(tx));
        })
        .await;

        while let Some(req) = rx.recv().await {
            log::info!("Node request: {:?}", req);

            let ctx = ctx.clone();
            let res = match req {
                LightClientRequest::Starknet(starknet_req) => {
                    use StarknetRequest::*;

                    match starknet_req {
                        GetBlockWithTxHashes(params) => {
                            StarknetRequest::get_block_with_tx_hashes(&client, ctx, params).await
                        }
                        GetBlockWithTxs(params) => {
                            StarknetRequest::get_block_with_txs(&client, ctx, params).await
                        }
                        GetStateUpdate(params) => {
                            StarknetRequest::get_state_update(&client, ctx, params).await
                        }
                        GetStorageAt(params) => {
                            StarknetRequest::get_storage_at(&client, ctx, params).await
                        }
                        GetTransactionByHash(params) => {
                            StarknetRequest::get_transaction_by_hash(&client, ctx, params).await
                        }
                        GetTransactionByBlockIdAndIndex(params) => {
                            StarknetRequest::get_transaction_by_block_id_and_index(
                                &client, ctx, params,
                            )
                            .await
                        }
                        GetTransactionReceipt(params) => {
                            StarknetRequest::get_transaction_receipt(&client, ctx, params).await
                        }
                        GetClass(params) => StarknetRequest::get_class(&client, ctx, params).await,
                        GetClassHashAt(params) => {
                            StarknetRequest::get_class_hash_at(&client, ctx, params).await
                        }
                        GetClassAt(params) => {
                            StarknetRequest::get_class_at(&client, ctx, params).await
                        }
                        GetBlockTransactionCount(params) => {
                            StarknetRequest::get_block_transaction_count(&client, ctx, params).await
                        }
                        Call(params) => StarknetRequest::get_call(&client, ctx, params).await,
                        EstimateFee(params) => {
                            StarknetRequest::estimate_fee(&client, ctx, params).await
                        }
                        BlockNumber => StarknetRequest::block_number(&client, ctx).await,
                        BlockHashAndNumber => {
                            StarknetRequest::block_hash_and_number(&client, ctx).await
                        }
                        ChainId => StarknetRequest::chain_id(&client, ctx).await,
                        PendingTransactions => {
                            StarknetRequest::pending_transactions(&client, ctx).await
                        }
                        Syncing => StarknetRequest::syncing(&client, ctx).await,
                        GetEvents(params) => {
                            StarknetRequest::get_events(&client, ctx, params).await
                        }
                        GetNonce(params) => StarknetRequest::get_nonce(&client, ctx, params).await,
                        L1ToL2Messages(params) => {
                            StarknetRequest::l1_to_l2_messages(&client, ctx, params).await
                        }
                        L1ToL2MessageNonce => {
                            StarknetRequest::l1_to_l2_message_nonce(&client, ctx).await
                        }
                        L1ToL2MessageCancellations(params) => {
                            StarknetRequest::l1_to_l2_message_cancellations(&client, ctx, params)
                                .await
                        }
                        L2ToL1Messages(params) => {
                            StarknetRequest::l2_to_l1_messages(&client, ctx, params).await
                        }
                        AddDeclareTransaction(params) => {
                            StarknetRequest::add_declare_transaction(&client, ctx, params).await
                        }
                        AddDeployAccountTransaction(params) => {
                            StarknetRequest::add_deploy_account_transaction(&client, ctx, params)
                                .await
                        }
                        GetContractStorageProof(params) => {
                            StarknetRequest::get_contract_storage_proof(&client, ctx, params).await
                        }
                        AddInvokeTransaction(params) => {
                            StarknetRequest::add_invoke_transaction(&client, ctx, params).await
                        }
                    }
                }
                LightClientRequest::Ethereum(ethereum_req) => {
                    use EthRequest::*;

                    match ethereum_req {
                        GetBlockNumber => EthRequest::get_block_number(&client, ctx).await,
                    }
                }
            };

            if let Err(e) = res {
                log::error!("{e}");
            }
        }

        Ok::<(), Error>(())
    });

    Ok(())
}

#[derive(Component)]
pub struct LightClient {
    tx: Sender<LightClientRequest>,
}

impl LightClient {
    fn new(tx: Sender<LightClientRequest>) -> Self {
        Self { tx }
    }

    pub fn send(&self, req: LightClientRequest) -> Result<(), TrySendError<LightClientRequest>> {
        self.tx.try_send(req)
    }
}

// TODO: Should we expose it as a component bundle instead? Then, add systems to convert it as enum.
#[derive(Debug)]
pub enum LightClientRequest {
    Ethereum(EthRequest),
    Starknet(StarknetRequest),
}

#[derive(Component)]
pub struct BlockNumber {
    pub value: u64,
}

impl BlockNumber {
    fn new(value: u64) -> Self {
        Self { value }
    }
}

fn handle_light_client_error(In(result): In<Result<()>>) {
    if let Err(e) = result {
        log::error!("{e}");
    }
}

fn handle_request_error(In(result): In<Result<()>>) {
    if let Err(e) = result {
        log::error!("{e}");
    }
}
