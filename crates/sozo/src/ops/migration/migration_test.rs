use camino::Utf8PathBuf;
use dojo_test_utils::sequencer::{SequencerConfig, TestSequencer};
use dojo_world::manifest::Manifest;
use dojo_world::migration::strategy::prepare_for_migration;
use dojo_world::migration::world::WorldDiff;
use scarb::core::Config;
use scarb::ui::Verbosity;
use starknet::accounts::SingleOwnerAccount;
use starknet::core::chain_id;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::JsonRpcClient;
use starknet::signers::{LocalWallet, SigningKey};

use crate::ops::migration::execute_strategy;

#[tokio::test]
async fn test_migration_with_auto_mine() {
    let target_dir = Utf8PathBuf::from_path_buf("../../examples/ecs/target/dev".into()).unwrap();

    let sequencer = TestSequencer::start(SequencerConfig::default()).await;

    let account = SingleOwnerAccount::new(
        JsonRpcClient::new(HttpTransport::new(sequencer.url())),
        LocalWallet::from_signing_key(SigningKey::from_secret_scalar(
            sequencer.raw_account().private_key,
        )),
        sequencer.raw_account().account_address,
        chain_id::TESTNET,
    );

    let config = Config::builder(Utf8PathBuf::from_path_buf("../../examples/ecs/".into()).unwrap())
        .ui_verbosity(Verbosity::Quiet)
        .build()
        .unwrap();

    let manifest = Manifest::load_from_path(target_dir.join("manifest.json")).unwrap();
    let world = WorldDiff::compute(manifest, None);

    let mut migration = prepare_for_migration(None, target_dir, world).unwrap();
    execute_strategy(&mut migration, account, &config).await.unwrap();

    sequencer.stop().unwrap();
}

#[tokio::test]
async fn test_migration_with_block_time() {
    let target_dir = Utf8PathBuf::from_path_buf("../../examples/ecs/target/dev".into()).unwrap();

    let sequencer = TestSequencer::start(SequencerConfig { block_time: Some(1) }).await;

    let account = SingleOwnerAccount::new(
        JsonRpcClient::new(HttpTransport::new(sequencer.url())),
        LocalWallet::from_signing_key(SigningKey::from_secret_scalar(
            sequencer.raw_account().private_key,
        )),
        sequencer.raw_account().account_address,
        chain_id::TESTNET,
    );

    let config = Config::builder(Utf8PathBuf::from_path_buf("../../examples/ecs/".into()).unwrap())
        .ui_verbosity(Verbosity::Quiet)
        .build()
        .unwrap();

    let manifest = Manifest::load_from_path(target_dir.join("manifest.json")).unwrap();
    let world = WorldDiff::compute(manifest, None);

    let mut migration = prepare_for_migration(None, target_dir, world).unwrap();
    execute_strategy(&mut migration, account, &config).await.unwrap();

    sequencer.stop().unwrap();
}

#[ignore]
#[tokio::test]
async fn test_migration_from_remote() {
    let target_dir = Utf8PathBuf::from_path_buf("../../examples/ecs/target/dev".into()).unwrap();

    let sequencer = TestSequencer::start(SequencerConfig::default()).await;

    let account = SingleOwnerAccount::new(
        JsonRpcClient::new(HttpTransport::new(sequencer.url())),
        LocalWallet::from_signing_key(SigningKey::from_secret_scalar(
            sequencer.raw_account().private_key,
        )),
        sequencer.raw_account().account_address,
        chain_id::TESTNET,
    );

    let config = Config::builder(Utf8PathBuf::from_path_buf("../../examples/ecs/".into()).unwrap())
        .ui_verbosity(Verbosity::Quiet)
        .build()
        .unwrap();

    let manifest = Manifest::load_from_path(target_dir.clone()).unwrap();
    let world = WorldDiff::compute(manifest, None);

    let mut migration = prepare_for_migration(None, target_dir.clone(), world).unwrap();

    execute_strategy(&mut migration, account, &config).await.unwrap();

    let local_manifest = Manifest::load_from_path(target_dir.join("manifest.json")).unwrap();
    let remote_manifest = Manifest::from_remote(
        JsonRpcClient::new(HttpTransport::new(sequencer.url())),
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
