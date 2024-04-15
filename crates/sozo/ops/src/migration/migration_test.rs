use camino::Utf8Path;
use dojo_lang::compiler::{BASE_DIR, MANIFESTS_DIR};
use dojo_test_utils::compiler::build_test_config;
use dojo_test_utils::migration::prepare_migration;
use dojo_test_utils::sequencer::{
    get_default_test_starknet_config, SequencerConfig, StarknetConfig, TestSequencer,
};
use dojo_world::manifest::{BaseManifest, DeploymentManifest};
use dojo_world::migration::strategy::prepare_for_migration;
use dojo_world::migration::world::WorldDiff;
use dojo_world::migration::TxnConfig;
use scarb::ops;
use starknet::accounts::{ExecutionEncoding, SingleOwnerAccount};
use starknet::core::chain_id;
use starknet::core::types::{BlockId, BlockTag};
use starknet::macros::felt;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::JsonRpcClient;
use starknet::signers::{LocalWallet, SigningKey};

use crate::migration::execute_strategy;

#[tokio::test(flavor = "multi_thread")]
async fn migrate_with_auto_mine() {
    let config = build_test_config("../../../examples/spawn-and-move/Scarb.toml").unwrap();
    let ws = ops::read_workspace(config.manifest_path(), &config)
        .unwrap_or_else(|op| panic!("Error building workspace: {op:?}"));

    let base_dir = "../../../examples/spawn-and-move";
    let target_dir = format!("{}/target/dev", base_dir);
    let mut migration = prepare_migration(base_dir.into(), target_dir.into()).unwrap();

    let sequencer =
        TestSequencer::start(SequencerConfig::default(), get_default_test_starknet_config()).await;

    let mut account = sequencer.account();
    account.set_block_id(BlockId::Tag(BlockTag::Pending));

    execute_strategy(&ws, &mut migration, &account, None).await.unwrap();

    sequencer.stop().unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn migrate_with_block_time() {
    let config = build_test_config("../../../examples/spawn-and-move/Scarb.toml").unwrap();
    let ws = ops::read_workspace(config.manifest_path(), &config)
        .unwrap_or_else(|op| panic!("Error building workspace: {op:?}"));

    let base = "../../../examples/spawn-and-move";
    let target_dir = format!("{}/target/dev", base);
    let mut migration = prepare_migration(base.into(), target_dir.into()).unwrap();

    let sequencer = TestSequencer::start(
        SequencerConfig { block_time: Some(1000), ..Default::default() },
        get_default_test_starknet_config(),
    )
    .await;

    let mut account = sequencer.account();
    account.set_block_id(BlockId::Tag(BlockTag::Pending));

    execute_strategy(&ws, &mut migration, &account, None).await.unwrap();
    sequencer.stop().unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn migrate_with_small_fee_multiplier_will_fail() {
    let config = build_test_config("../../../examples/spawn-and-move/Scarb.toml").unwrap();
    let ws = ops::read_workspace(config.manifest_path(), &config)
        .unwrap_or_else(|op| panic!("Error building workspace: {op:?}"));

    let base = "../../../examples/spawn-and-move";
    let target_dir = format!("{}/target/dev", base);
    let mut migration = prepare_migration(base.into(), target_dir.into()).unwrap();

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

    assert!(execute_strategy(
        &ws,
        &mut migration,
        &account,
        Some(TxnConfig { fee_estimate_multiplier: Some(0.2f64), wait: false, receipt: false }),
    )
    .await
    .is_err());
    sequencer.stop().unwrap();
}

#[test]
fn migrate_world_without_seed_will_fail() {
    let profile_name = "dev";
    let base = "../../../examples/spawn-and-move";
    let target_dir = format!("{}/target/dev", base);
    let manifest = BaseManifest::load_from_path(
        &Utf8Path::new(base).to_path_buf().join(MANIFESTS_DIR).join(profile_name).join(BASE_DIR),
    )
    .unwrap();
    let world = WorldDiff::compute(manifest, None);
    let res = prepare_for_migration(None, None, &Utf8Path::new(&target_dir).to_path_buf(), world);
    assert!(res.is_err_and(|e| e.to_string().contains("Missing seed for World deployment.")))
}

#[tokio::test]
async fn migration_from_remote() {
    let config = build_test_config("../../../examples/spawn-and-move/Scarb.toml").unwrap();
    let ws = ops::read_workspace(config.manifest_path(), &config)
        .unwrap_or_else(|op| panic!("Error building workspace: {op:?}"));
    let base = "../../../examples/spawn-and-move";
    let target_dir = format!("{}/target/dev", base);

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

    let profile_name = ws.current_profile().unwrap().to_string();

    let manifest = BaseManifest::load_from_path(
        &Utf8Path::new(base).to_path_buf().join(MANIFESTS_DIR).join(&profile_name).join(BASE_DIR),
    )
    .unwrap();

    let world = WorldDiff::compute(manifest, None);

    let mut migration = prepare_for_migration(
        None,
        Some(felt!("0x12345")),
        &Utf8Path::new(&target_dir).to_path_buf(),
        world,
    )
    .unwrap();

    execute_strategy(&ws, &mut migration, &account, None).await.unwrap();

    let local_manifest = BaseManifest::load_from_path(
        &Utf8Path::new(base).to_path_buf().join(MANIFESTS_DIR).join(&profile_name).join(BASE_DIR),
    )
    .unwrap();

    let remote_manifest = DeploymentManifest::load_from_remote(
        JsonRpcClient::new(HttpTransport::new(sequencer.url())),
        migration.world_address().unwrap(),
    )
    .await
    .unwrap();

    sequencer.stop().unwrap();

    assert_eq!(local_manifest.world.inner.class_hash, remote_manifest.world.inner.class_hash);
    assert_eq!(local_manifest.models.len(), remote_manifest.models.len());
}
