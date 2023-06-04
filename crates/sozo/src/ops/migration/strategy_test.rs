use camino::Utf8PathBuf;
use dojo_test_utils::sequencer::Sequencer;

use crate::ops::migration::config::{EnvironmentConfig, WorldConfig};
use crate::ops::migration::strategy::{execute_migration, prepare_for_migration};
use crate::ops::migration::world::WorldDiff;

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
    execute_migration(&mut migration, env_config.migrator().await.unwrap()).await.unwrap();

    sequencer.stop().unwrap();
}
