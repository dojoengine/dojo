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
use std::process::exit;

use beerus_core::config::Config;
use beerus_core::lightclient::beerus::BeerusLightClient;
use beerus_core::lightclient::ethereum::helios_lightclient::HeliosLightClient;
use beerus_core::lightclient::starknet::StarkNetLightClientImpl;
use bevy::app::Plugin;
use bevy::ecs::system::ResMut;
use bevy::log;
use bevy_tokio_tasks::{TokioTasksPlugin, TokioTasksRuntime};

/// Plugin to manage Ethereum/Starknet light client.
pub struct LightClientPlugin;

impl Plugin for LightClientPlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        app.add_plugin(TokioTasksPlugin::default())
            .add_plugin(bevy::core::TaskPoolPlugin::default())
            .add_startup_system(setup);
    }
}

fn setup(runtime: ResMut<'_, TokioTasksRuntime>) {
    log::info!("Starting...");
    // TODO: Env doesn't exist on browser
    let config = Config::from_env();

    runtime.spawn_background_task(|_ctx| async move {
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
            }
            Err(e) => {
                log::error!("{e}");
            }
        }
    });
}
