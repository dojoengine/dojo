use camino::Utf8PathBuf;
use dojo_test_utils::sequencer::Sequencer;
use dojo_world::config::{EnvironmentConfig, WorldConfig};
use dojo_world::migration::world::WorldDiff;

use crate::ops::migrate::prepare_for_migration;

#[tokio::test]
async fn test_migration() {
    let target_dir = Utf8PathBuf::from_path_buf("../../examples/ecs/target/dev".into()).unwrap();

    let sequencer = Sequencer::start().await;
    let account = sequencer.account();
    let env_config = EnvironmentConfig {
        rpc: Some(sequencer.url()),
        account_address: Some(account.address),
        private_key: Some(account.private_key),
        ..EnvironmentConfig::default()
    };

    let world = WorldDiff::from_path(target_dir.clone(), &WorldConfig::default(), &env_config)
        .await
        .unwrap();

    let mut migration = prepare_for_migration(target_dir, world, WorldConfig::default()).unwrap();
    migration.execute(env_config.migrator().await.unwrap()).await.unwrap();

    sequencer.stop().unwrap();
}
