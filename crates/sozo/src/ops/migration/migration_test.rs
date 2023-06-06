use camino::Utf8PathBuf;
use dojo_test_utils::sequencer::Sequencer;
use dojo_world::manifest::Manifest;
use scarb::core::Config;
use scarb::ui::Verbosity;

use crate::ops::migration::config::{EnvironmentConfig, WorldConfig};
use crate::ops::migration::execute_strategy;
use crate::ops::migration::strategy::prepare_for_migration;
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

    let config = Config::builder(Utf8PathBuf::from_path_buf("../../examples/ecs/".into()).unwrap())
        .ui_verbosity(Verbosity::Normal)
        .build()
        .unwrap();

    let world =
        WorldDiff::from_path(target_dir.clone(), &WorldConfig::default(), &env_config, &config)
            .await
            .unwrap();

    let mut migration = prepare_for_migration(target_dir, world, WorldConfig::default()).unwrap();
    execute_strategy(&mut migration, env_config.migrator().await.unwrap(), &config).await.unwrap();

    sequencer.stop().unwrap();
}

#[tokio::test]
async fn test_migration_from_remote() {
    let target_dir = Utf8PathBuf::from_path_buf("../../examples/ecs/target/dev".into()).unwrap();

    let sequencer = Sequencer::start().await;
    let account = sequencer.account();
    let env_config = EnvironmentConfig {
        rpc: Some(sequencer.url()),
        account_address: Some(account.address),
        private_key: Some(account.private_key),
        ..EnvironmentConfig::default()
    };

    let config = Config::builder(Utf8PathBuf::from_path_buf("../../examples/ecs/".into()).unwrap())
        .ui_verbosity(Verbosity::Quiet)
        .build()
        .unwrap();

    let world =
        WorldDiff::from_path(target_dir.clone(), &WorldConfig::default(), &env_config, &config)
            .await
            .unwrap();

    let mut migration =
        prepare_for_migration(target_dir.clone(), world, WorldConfig::default()).unwrap();

    execute_strategy(&mut migration, env_config.migrator().await.unwrap(), &config).await.unwrap();

    let _local_manifest = Manifest::load_from_path(target_dir.join("manifest.json")).unwrap();
    let _remote_manifest = Manifest::from_remote(
        env_config.provider().unwrap(),
        migration.world_address().unwrap(),
        None,
    )
    .await
    .unwrap();

    sequencer.stop().unwrap();
}
