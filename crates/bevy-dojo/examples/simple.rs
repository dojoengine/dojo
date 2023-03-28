use bevy_app::App;
use bevy_dojo::IndexerPlugin;

fn main() {
    App::new().set_runner(runner).add_plugin(IndexerPlugin).run();
}

fn runner(mut app: App) {
    loop {
        app.update();
    }
}
