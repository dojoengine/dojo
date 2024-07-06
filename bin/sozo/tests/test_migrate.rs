mod utils;

use camino::Utf8PathBuf;
use dojo_test_utils::compiler;
use katana_runner::KatanaRunner;
use scarb::compiler::Profile;
use starknet::accounts::Account;
use starknet::core::types::{BlockId, BlockTag};
use utils::snapbox::get_snapbox;

#[tokio::test(flavor = "multi_thread")]
async fn migrate_dry_run() {
    let source_project_dir = Utf8PathBuf::from("../../examples/spawn-and-move/");
    let dojo_core_path = Utf8PathBuf::from("../../crates/dojo-core");

    let config = compiler::copy_tmp_config(&source_project_dir, &dojo_core_path, Profile::DEV);

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
    assert!(format!("{:?}", assert.get_output()).contains("Migration Strategy"));
    assert!(format!("{:?}", assert.get_output()).contains("# Base Contract"));
    assert!(format!("{:?}", assert.get_output()).contains("# Models (8)"));
    assert!(format!("{:?}", assert.get_output()).contains("# World"));
    assert!(format!("{:?}", assert.get_output()).contains("# Contracts (3)"));
}
