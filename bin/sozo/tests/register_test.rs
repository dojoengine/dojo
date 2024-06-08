mod utils;

use camino::Utf8PathBuf;
use dojo_test_utils::compiler;
use dojo_test_utils::migration::prepare_migration;
use dojo_world::metadata::dojo_metadata_from_workspace;
use dojo_world::migration::TxnConfig;
use katana_runner::KatanaRunner;
use scarb::ops;
use sozo_ops::migration::execute_strategy;
use starknet::accounts::Account;
use starknet::core::types::{BlockId, BlockTag};
use utils::snapbox::get_snapbox;

#[tokio::test(flavor = "multi_thread")]
async fn reregister_models() {
    let source_project_dir = Utf8PathBuf::from("../../examples/spawn-and-move/");
    let dojo_core_path = Utf8PathBuf::from("../../crates/dojo-core");

    let config = compiler::copy_tmp_config(&source_project_dir, &dojo_core_path);

    let ws = ops::read_workspace(config.manifest_path(), &config)
        .unwrap_or_else(|op| panic!("Error building workspace: {op:?}"));
    let dojo_metadata = dojo_metadata_from_workspace(&ws);

    let target_path =
        ws.target_dir().path_existent().unwrap().join(ws.config().profile().to_string());

    let migration = prepare_migration(
        source_project_dir.into(),
        target_path.into(),
        dojo_metadata.skip_migration,
    )
    .unwrap();

    let sequencer = KatanaRunner::new().expect("Failed to start runner.");

    let mut account = sequencer.account(0);
    account.set_block_id(BlockId::Tag(BlockTag::Pending));

    execute_strategy(&ws, &migration, &account, TxnConfig::init_wait()).await.unwrap();
    let world_address = &format!("0x{:x}", &migration.world_address().unwrap());
    let account_address = &format!("0x{:x}", account.address());
    let private_key = &format!("0x{:x}", sequencer.account_data(0).1.private_key);
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
