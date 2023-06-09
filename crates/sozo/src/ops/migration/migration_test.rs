use camino::Utf8PathBuf;
use dojo_test_utils::sequencer::TestSequencer;
use dojo_world::manifest::Manifest;
use dojo_world::migration::world::WorldDiff;
use scarb::core::Config;
use scarb::ui::Verbosity;

use crate::ops::migration::config::{EnvironmentConfig, WorldConfig};
use crate::ops::migration::execute_strategy;
use crate::ops::migration::strategy::prepare_for_migration;

#[tokio::test]
async fn test_migration() {
    let target_dir = Utf8PathBuf::from_path_buf("../../examples/ecs/target/dev".into()).unwrap();

    let sequencer = TestSequencer::start().await;

    let env_config = EnvironmentConfig {
        rpc: Some(sequencer.url()),
        private_key: Some(sequencer.raw_account().private_key),
        account_address: Some(sequencer.raw_account().account_address),
        ..EnvironmentConfig::default()
    };

    let config = Config::builder(Utf8PathBuf::from_path_buf("../../examples/ecs/".into()).unwrap())
        .ui_verbosity(Verbosity::Quiet)
        .build()
        .unwrap();

    let manifest = Manifest::load_from_path(target_dir.join("manifest.json")).unwrap();
    let world = WorldDiff::compute(manifest, None);

    let mut migration = prepare_for_migration(target_dir, world, WorldConfig::default()).unwrap();
    execute_strategy(&mut migration, env_config.migrator().await.unwrap(), &config).await.unwrap();

    sequencer.stop().unwrap();
}

#[ignore]
#[tokio::test]
async fn test_migration_from_remote() {
    let target_dir = Utf8PathBuf::from_path_buf("../../examples/ecs/target/dev".into()).unwrap();

    let sequencer = TestSequencer::start().await;

    let env_config = EnvironmentConfig {
        rpc: Some(sequencer.url()),
        private_key: Some(sequencer.raw_account().private_key),
        account_address: Some(sequencer.raw_account().account_address),
        ..EnvironmentConfig::default()
    };

    let config = Config::builder(Utf8PathBuf::from_path_buf("../../examples/ecs/".into()).unwrap())
        .ui_verbosity(Verbosity::Quiet)
        .build()
        .unwrap();

    let manifest = Manifest::load_from_path(target_dir.clone()).unwrap();
    let world = WorldDiff::compute(manifest, None);

    let mut migration =
        prepare_for_migration(target_dir.clone(), world, WorldConfig::default()).unwrap();

    execute_strategy(&mut migration, env_config.migrator().await.unwrap(), &config).await.unwrap();

    let local_manifest = Manifest::load_from_path(target_dir.join("manifest.json")).unwrap();
    let remote_manifest = Manifest::from_remote(
        env_config.provider().unwrap(),
        migration.world_address().unwrap(),
        None,
    )
    .await
    .unwrap();

    sequencer.stop().unwrap();

    assert_eq!(local_manifest.world.class_hash, remote_manifest.world.class_hash);
    assert_eq!(local_manifest.executor.class_hash, remote_manifest.executor.class_hash);
    assert_eq!(local_manifest.components.len(), remote_manifest.components.len());
    assert_eq!(local_manifest.systems.len(), remote_manifest.systems.len());
}
