use camino::Utf8PathBuf;
use dojo_test_utils::compiler::build_test_config;
use dojo_test_utils::migration::prepare_migration;
use dojo_test_utils::sequencer::{
    get_default_test_starknet_config, SequencerConfig, StarknetConfig, TestSequencer,
};
use dojo_world::manifest::Manifest;
use dojo_world::migration::strategy::prepare_for_migration;
use dojo_world::migration::world::WorldDiff;
use scarb::ops;
use starknet::accounts::{ExecutionEncoding, SingleOwnerAccount};
use starknet::core::chain_id;
use starknet::core::types::{BlockId, BlockTag};
use starknet::macros::felt;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::JsonRpcClient;
use starknet::signers::{LocalWallet, SigningKey};

use crate::commands::options::transaction::TransactionOptions;
use crate::ops::migration::execute_strategy;

#[tokio::test(flavor = "multi_thread")]
async fn migrate_with_auto_mine() {
    let config = build_test_config("../../examples/spawn-and-move/Scarb.toml").unwrap();
    let ws = ops::read_workspace(config.manifest_path(), &config)
        .unwrap_or_else(|op| panic!("Error building workspace: {op:?}"));

    let migration = prepare_migration("../../examples/spawn-and-move/target/dev".into()).unwrap();

    let sequencer =
        TestSequencer::start(SequencerConfig::default(), get_default_test_starknet_config()).await;

    let mut account = sequencer.account();
    account.set_block_id(BlockId::Tag(BlockTag::Pending));

    execute_strategy(&ws, &migration, &account, None).await.unwrap();

    sequencer.stop().unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn migrate_with_block_time() {
    let config = build_test_config("../../examples/spawn-and-move/Scarb.toml").unwrap();
    let ws = ops::read_workspace(config.manifest_path(), &config)
        .unwrap_or_else(|op| panic!("Error building workspace: {op:?}"));

    let migration = prepare_migration("../../examples/spawn-and-move/target/dev".into()).unwrap();

    let sequencer = TestSequencer::start(
        SequencerConfig { block_time: Some(1000), ..Default::default() },
        get_default_test_starknet_config(),
    )
    .await;

    let mut account = sequencer.account();
    account.set_block_id(BlockId::Tag(BlockTag::Pending));

    execute_strategy(&ws, &migration, &account, None).await.unwrap();
    sequencer.stop().unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn migrate_with_small_fee_multiplier_will_fail() {
    let config = build_test_config("../../examples/spawn-and-move/Scarb.toml").unwrap();
    let ws = ops::read_workspace(config.manifest_path(), &config)
        .unwrap_or_else(|op| panic!("Error building workspace: {op:?}"));

    let migration = prepare_migration("../../examples/spawn-and-move/target/dev".into()).unwrap();

    let sequencer = TestSequencer::start(
        Default::default(),
        StarknetConfig { disable_fee: false, ..Default::default() },
    )
    .await;

    let account = SingleOwnerAccount::new(
        JsonRpcClient::new(HttpTransport::new(sequencer.url())),
        LocalWallet::from_signing_key(SigningKey::from_secret_scalar(
            sequencer.raw_account().private_key,
        )),
        sequencer.raw_account().account_address,
        chain_id::TESTNET,
        ExecutionEncoding::New,
    );

    assert!(
        execute_strategy(
            &ws,
            &migration,
            &account,
            Some(TransactionOptions { fee_estimate_multiplier: Some(0.2f64), wait: false }),
        )
        .await
        .is_err()
    );
    sequencer.stop().unwrap();
}

#[test]
fn migrate_world_without_seed_will_fail() {
    let target_dir =
        Utf8PathBuf::from_path_buf("../../examples/spawn-and-move/target/dev".into()).unwrap();
    let manifest = Manifest::load_from_path(target_dir.join("manifest.json")).unwrap();
    let world = WorldDiff::compute(manifest, None);
    let res = prepare_for_migration(None, None, target_dir, world);
    assert!(res.is_err_and(|e| e.to_string().contains("Missing seed for World deployment.")))
}

#[ignore]
#[tokio::test]
async fn migration_from_remote() {
    let config = build_test_config("../../examples/spawn-and-move/Scarb.toml").unwrap();
    let ws = ops::read_workspace(config.manifest_path(), &config)
        .unwrap_or_else(|op| panic!("Error building workspace: {op:?}"));
    let target_dir =
        Utf8PathBuf::from_path_buf("../../examples/spawn-and-move/target/dev".into()).unwrap();

    let sequencer =
        TestSequencer::start(SequencerConfig::default(), get_default_test_starknet_config()).await;

    let account = SingleOwnerAccount::new(
        JsonRpcClient::new(HttpTransport::new(sequencer.url())),
        LocalWallet::from_signing_key(SigningKey::from_secret_scalar(
            sequencer.raw_account().private_key,
        )),
        sequencer.raw_account().account_address,
        chain_id::TESTNET,
        ExecutionEncoding::New,
    );

    let manifest = Manifest::load_from_path(target_dir.join("manifest.json")).unwrap();
    let world = WorldDiff::compute(manifest, None);

    let migration =
        prepare_for_migration(None, Some(felt!("0x12345")), target_dir.clone(), world).unwrap();

    execute_strategy(&ws, &migration, &account, None).await.unwrap();

    let local_manifest = Manifest::load_from_path(target_dir.join("manifest.json")).unwrap();
    let remote_manifest = Manifest::load_from_remote(
        JsonRpcClient::new(HttpTransport::new(sequencer.url())),
        migration.world_address().unwrap(),
    )
    .await
    .unwrap();

    sequencer.stop().unwrap();

    assert_eq!(local_manifest.world.class_hash, remote_manifest.world.class_hash);
    assert_eq!(local_manifest.executor.class_hash, remote_manifest.executor.class_hash);
    assert_eq!(local_manifest.models.len(), remote_manifest.models.len());
}
