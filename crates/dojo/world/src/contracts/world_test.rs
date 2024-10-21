use dojo_test_utils::compiler::CompilerTestSetup;
use dojo_test_utils::migration::{copy_spawn_and_move_db, prepare_migration_with_world_and_seed};
use katana_runner::RunnerCtx;
use scarb::compiler::Profile;
use starknet::accounts::ConnectedAccount;
use starknet::core::types::{BlockId, BlockTag};

use super::WorldContractReader;

#[tokio::test(flavor = "multi_thread")]
#[katana_runner::test(db_dir = copy_spawn_and_move_db().as_str())]
async fn test_world_contract_reader(sequencer: &RunnerCtx) {
    let setup = CompilerTestSetup::from_examples("../dojo-core", "../../examples/");
    let config = setup.build_test_config("spawn-and-move", Profile::DEV);

    let manifest_dir = config.manifest_path().parent().unwrap();
    let target_dir = manifest_dir.join("target").join("dev");

    let mut account = sequencer.account(0);
    account.set_block_id(BlockId::Tag(BlockTag::Pending));

    let provider = account.provider();

    let (strat, _) = prepare_migration_with_world_and_seed(
        manifest_dir.to_path_buf(),
        target_dir.to_path_buf(),
        None,
        "dojo_examples",
        "dojo_examples",
    )
    .unwrap();

    let _world = WorldContractReader::new(strat.world_address, provider);
}
