use bevy::log::LogPlugin;
use bevy::prelude::*;
use bevy_dojo::prelude::*;

fn main() {
    App::new()
        .set_runner(|mut app| loop {
            app.update()
        })
        .add_plugin(LogPlugin::default())
        // Set up Starknet light client plugin.
        .add_plugin(LightClientPlugin)
        .run();
}
