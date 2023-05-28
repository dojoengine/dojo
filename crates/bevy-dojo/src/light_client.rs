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
            .add_startup_system(start_light_client.pipe(handle_errors));
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
                        GetBlockWithTxHashes => {
                            StarknetRequest::get_block_with_tx_hashes(&client, ctx).await
                        }
                        BlockNumber(e) => StarknetRequest::block_number(&client, ctx, e).await,
                        _ => {
                            unreachable!()
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

fn handle_errors(In(result): In<Result<()>>) {
    if let Err(e) = result {
        log::error!("{e}");
    }
}
