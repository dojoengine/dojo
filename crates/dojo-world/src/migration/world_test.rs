use camino::Utf8PathBuf;
use dojo_test_utils::sequencer::Sequencer;

use crate::migration::world::World;
use crate::{EnvironmentConfig, WorldConfig};

#[tokio::test]
async fn test_migration() {
    let target_dir = Utf8PathBuf::from_path_buf("../../examples/ecs/target/dev".into()).unwrap();

    let sequencer = Sequencer::start().await;
    let account = sequencer.account();
    let world = World::from_path(
        target_dir.clone(),
        WorldConfig::default(),
        EnvironmentConfig {
            rpc: Some(sequencer.url()),
            account_address: Some(account.address),
            private_key: Some(account.private_key),
            ..EnvironmentConfig::default()
        },
    )
    .await
    .unwrap();

    let mut migration = world.prepare_for_migration(target_dir).await.unwrap();
    migration.execute().await.unwrap();

    sequencer.stop().unwrap();
}
