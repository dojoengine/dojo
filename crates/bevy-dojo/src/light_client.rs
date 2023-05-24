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

pub mod prelude {
    pub use crate::light_client::*;
}

use std::process::exit;

use beerus_core::config::Config;
use beerus_core::lightclient::beerus::BeerusLightClient;
use beerus_core::lightclient::ethereum::helios_lightclient::HeliosLightClient;
use beerus_core::lightclient::starknet::StarkNetLightClientImpl;
use bevy::app::Plugin;
use bevy::ecs::component::Component;
use bevy::ecs::system::ResMut;
use bevy::log;
use bevy_tokio_tasks::{TokioTasksPlugin, TokioTasksRuntime};
use tokio::sync::mpsc::error::TrySendError;
use tokio::sync::mpsc::Sender;

/// Plugin to manage Ethereum/Starknet light client.
pub struct LightClientPlugin;

impl Plugin for LightClientPlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        app.add_event::<StarknetBlockNumber>()
            .add_plugin(TokioTasksPlugin::default())
            .add_startup_system(start_beerus);
    }
}

fn start_beerus(runtime: ResMut<'_, TokioTasksRuntime>) {
    log::info!("Starting...");
    let config = Config::from_env();
    let (tx, mut rx) = tokio::sync::mpsc::channel::<NodeRequest>(1);

    runtime.spawn_background_task(|mut ctx| async move {
        log::info!("creating ethereum(helios) lightclient...");
        let ethereum_lightclient = match HeliosLightClient::new(config.clone()).await {
            Ok(ethereum_lightclient) => ethereum_lightclient,
            Err(err) => {
                log::error! {"{}", err};
                exit(1);
            }
        };

        log::info!("creating starknet lightclient...");
        let starknet_lightclient = match StarkNetLightClientImpl::new(&config) {
            Ok(starknet_lightclient) => starknet_lightclient,
            Err(err) => {
                log::error! {"{}", err};
                exit(1);
            }
        };

        log::info!("creating beerus lightclient");
        let mut beerus = BeerusLightClient::new(
            config,
            Box::new(ethereum_lightclient),
            Box::new(starknet_lightclient),
        );

        match beerus.start().await {
            Ok(_) => {
                log::info!("Light client started");
                ctx.run_on_main_thread(move |ctx| {
                    ctx.world.spawn(NodeClient::new(tx));
                })
                .await;

                while let Some(req) = rx.recv().await {
                    match req {
                        NodeRequest::Starknet(starknet_req) => match starknet_req {
                            StarknetRequest::BlockNumber => {
                                let res = beerus.starknet_lightclient.block_number().await;
                                log::info!("starknet__get_block_number: {:?}", res);

                                // TODO: spawn response as component
                            }
                        },
                    }
                }
            }
            Err(e) => {
                log::error!("{e}");
            }
        }
    });
}

#[derive(Component)]
pub struct NodeClient {
    tx: Sender<NodeRequest>,
}

impl NodeClient {
    fn new(tx: Sender<NodeRequest>) -> Self {
        Self { tx }
    }

    pub fn request(&self, req: NodeRequest) -> Result<(), TrySendError<NodeRequest>> {
        self.tx.try_send(req)
    }
}

// TODO: Support all methods
pub enum NodeRequest {
    Starknet(StarknetRequest),
    // Ethereum(EthereumRequest),
}

impl NodeRequest {
    pub fn starknet_block_number() -> Self {
        Self::Starknet(StarknetRequest::BlockNumber)
    }
}

pub enum StarknetRequest {
    BlockNumber,
}

// pub enum EthereumRequest {
//     BlockNumber,
// }

pub enum StarknetResponse {
    BlockNumber(u32),
}

// Events

pub struct StarknetBlockNumber;
