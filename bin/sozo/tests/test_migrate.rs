use std::fs;

use dojo_test_utils::compiler::CompilerTestSetup;
use dojo_test_utils::migration::copy_spawn_and_move_db;
use katana_runner::{KatanaRunner, RunnerCtx};
use scarb::compiler::Profile;
use starknet::accounts::Account;
use starknet::core::types::{BlockId, BlockTag};
use utils::snapbox::get_snapbox;

mod utils;

#[tokio::test(flavor = "multi_thread")]
async fn migrate_dry_run() {
    let setup = CompilerTestSetup::from_examples("../../crates/dojo-core", "../../examples/");
    let config = setup.build_test_config("spawn-and-move", Profile::DEV);

    let sequencer = KatanaRunner::new().expect("Failed to start runner.");

    let mut account = sequencer.account(0);
    account.set_block_id(BlockId::Tag(BlockTag::Pending));

    let account_address = &format!("0x{:x}", account.address());
    let private_key =
        &format!("0x{:x}", sequencer.account_data(0).private_key.as_ref().unwrap().secret_scalar());
    let rpc_url = &sequencer.url().to_string();

    let args_vec = [
        "migrate",
        "plan",
        "--account-address",
        account_address,
        "--rpc-url",
        rpc_url,
        "--private-key",
        private_key,
        "--manifest-path",
        config.manifest_path().as_ref(),
    ];

    let assert = get_snapbox().args(args_vec.iter()).assert().success();
    let output = format!("{:#?}", assert.get_output());

    dbg!("{}", &output);

    assert!(output.contains("Migration Strategy"));
    assert!(output.contains("# Base Contract"));
    assert!(output.contains("# Models (10)"));
    assert!(output.contains("# World"));
    assert!(output.contains("# Contracts (4)"));
}

#[tokio::test(flavor = "multi_thread")]
#[katana_runner::test(db_dir = copy_spawn_and_move_db().as_str())]
async fn test_migrate_then_upgrade(sequencer: &RunnerCtx) {
    let setup = CompilerTestSetup::from_examples("../../crates/dojo-core", "../../examples/");
    let config = setup.build_test_config("spawn-and-move", Profile::DEV);
    let tmp_dir = config.manifest_path().parent().unwrap();

    let mut account = sequencer.account(0);
    account.set_block_id(BlockId::Tag(BlockTag::Pending));

    let rpc_url = &sequencer.url().to_string();

    // Modify the actions contracts to have a new class hash.
    let actions_path = tmp_dir.join("src/actions.cairo");
    let mut actions_content = fs::read_to_string(&actions_path).unwrap();
    actions_content = actions_content.replace("quantity: 100", "quantity: 200");
    fs::write(&actions_path, actions_content).unwrap();

    let build_vec = ["build", "--manifest-path", config.manifest_path().as_ref()];

    get_snapbox().args(build_vec.iter()).assert().success();

    let plan_args = [
        "migrate",
        "plan",
        "--rpc-url",
        rpc_url,
        "--manifest-path",
        config.manifest_path().as_ref(),
    ];

    let plan_assert = get_snapbox().args(plan_args.iter()).assert().success();
    let plan_output = format!("{:#?}", plan_assert.get_output());

    assert!(plan_output.contains("Contracts (1)"));
}
