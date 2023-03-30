use std::process::exit;

use beerus_core::config::Config;
use beerus_core::lightclient::beerus::BeerusLightClient;
use beerus_core::lightclient::ethereum::helios_lightclient::HeliosLightClient;
use beerus_core::lightclient::starknet::StarkNetLightClientImpl;
use bevy::app::Plugin;
use bevy::core::TaskPoolPlugin;
use bevy::ecs::component::Component;
use bevy::ecs::system::Commands;
use bevy::log;
use bevy::tasks::{AsyncComputeTaskPool, Task};

pub struct LightClientPlugin;

impl Plugin for LightClientPlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        app.add_plugin(TaskPoolPlugin::default()).add_startup_system(setup);
    }
}

fn setup(mut commands: Commands) {
    log::info!("Starting...");
    // TODO: Env doesn't exist on browser
    let config = Config::from_env();

    let thread_pool = AsyncComputeTaskPool::get();
    let task = thread_pool.spawn(async move {
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

    commands.spawn(LightClientSync(task));
}

#[derive(Component)]
struct LightClientSync(Task<()>);
