use bevy_app::{App, Plugin};

pub struct IndexerPlugin;

impl Plugin for IndexerPlugin {
    fn build(&self, app: &mut App) {
        app.add_startup_system(setup);
    }
}

fn setup() {
    println!("Spawn async task for Apibara client: Loop message stream");
}
