mod utils;

use camino::Utf8PathBuf;
use dojo_test_utils::compiler::CompilerTestSetup;
use dojo_test_utils::migration::{copy_spawn_and_move_db, prepare_migration_with_world_and_seed};
use katana_runner::RunnerCtx;
use scarb::compiler::Profile;
use scarb::ops;
use starknet::accounts::Account;
use starknet::core::types::{BlockId, BlockTag};
use utils::snapbox::get_snapbox;

#[tokio::test(flavor = "multi_thread")]
#[katana_runner::test(db_dir = copy_spawn_and_move_db().as_str())]
async fn reregister_models(sequencer: &RunnerCtx) {
    let setup = CompilerTestSetup::from_examples("../../crates/dojo/core", "../../examples/");
    let config = setup.build_test_config("spawn-and-move", Profile::DEV);

    let ws = ops::read_workspace(config.manifest_path(), &config)
        .unwrap_or_else(|op| panic!("Error building workspace: {op:?}"));

    let manifest_path = Utf8PathBuf::from(config.manifest_path().parent().unwrap());
    let target_path =
        ws.target_dir().path_existent().unwrap().join(ws.config().profile().to_string());

    let (strat, _) = prepare_migration_with_world_and_seed(
        manifest_path,
        target_path,
        None,
        "dojo_examples",
        "dojo_examples",
    )
    .unwrap();

    let mut account = sequencer.account(0);
    account.set_block_id(BlockId::Tag(BlockTag::Pending));

    let world_address = &format!("0x{:x}", &strat.world_address);
    let account_address = &format!("0x{:x}", account.address());
    let private_key =
        &format!("0x{:x}", sequencer.account_data(0).private_key.as_ref().unwrap().secret_scalar());
    let rpc_url = &sequencer.url().to_string();

    let moves_model = strat.models.iter().find(|m| m.diff.tag == "dojo_examples-Moves").unwrap();
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
        "--manifest-path",
        config.manifest_path().as_ref(),
    ];

    let assert = get_snapbox().args(args_vec.iter()).assert().success();
    assert!(format!("{:?}", assert.get_output()).contains("No new models to register"));
}
