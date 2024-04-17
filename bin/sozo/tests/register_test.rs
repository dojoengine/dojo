mod utils;

use dojo_test_utils::compiler::build_test_config;
use dojo_test_utils::migration::prepare_migration;
use dojo_test_utils::sequencer::{
    get_default_test_starknet_config, SequencerConfig, TestSequencer,
};
use dojo_world::migration::TxnConfig;
use scarb::ops;
use sozo_ops::migration::execute_strategy;
use starknet::accounts::Account;
use starknet::core::types::{BlockId, BlockTag};
use utils::snapbox::get_snapbox;

#[tokio::test(flavor = "multi_thread")]
async fn reregister_models() {
    let config = build_test_config("../../examples/spawn-and-move/Scarb.toml").unwrap();
    let ws = ops::read_workspace(config.manifest_path(), &config)
        .unwrap_or_else(|op| panic!("Error building workspace: {op:?}"));

    let base_dir = "../../examples/spawn-and-move";
    let target_dir = format!("{}/target/dev", base_dir);
    let mut migration = prepare_migration(base_dir.into(), target_dir.into()).unwrap();

    let sequencer =
        TestSequencer::start(SequencerConfig::default(), get_default_test_starknet_config()).await;

    let mut account = sequencer.account();
    account.set_block_id(BlockId::Tag(BlockTag::Pending));

    execute_strategy(&ws, &mut migration, &account, TxnConfig::default()).await.unwrap();
    let world_address = &format!("0x{:x}", &migration.world_address().unwrap());
    let account_address = &format!("0x{:x}", account.address());
    let private_key = &format!("0x{:x}", sequencer.raw_account().private_key);
    let rpc_url = &sequencer.url().to_string();

    let moves_model =
        migration.models.iter().find(|m| m.diff.name == "dojo_examples::models::moves").unwrap();
    let moves_model_class_hash = &format!("0x{:x}", moves_model.diff.local_class_hash);
    let args_vec = [
        "register",
        "model",
        moves_model_class_hash,
        "--world",
        world_address,
        "--account-address",
        account_address,
        "--rpc-url",
        rpc_url,
        "--private-key",
        private_key,
    ];

    let assert = get_snapbox().args(args_vec.iter()).assert().success();
    assert!(format!("{:?}", assert.get_output()).contains("No new models to register"));
}
