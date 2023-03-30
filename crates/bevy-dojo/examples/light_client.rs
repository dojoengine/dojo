use bevy::app::App;
use bevy_dojo::LightClientPlugin;

fn main() {
    App::new().add_plugin(LightClientPlugin).set_runner(runner).run();
}

fn runner(mut app: App) {
    loop {
        app.update();
    }
}
