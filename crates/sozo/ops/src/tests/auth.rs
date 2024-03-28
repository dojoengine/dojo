use anyhow::Result;
use dojo_test_utils::compiler::build_test_config;
use dojo_test_utils::migration::prepare_migration;
use dojo_test_utils::sequencer::{
    get_default_test_starknet_config, SequencerConfig, TestSequencer,
};
use dojo_world::contracts::world::WorldContract;
use dojo_world::migration::TxConfig;
use scarb::ops;
use starknet::accounts::{Account, ConnectedAccount, SingleOwnerAccount};
use starknet::core::types::{BlockId, BlockTag};
use starknet::core::utils::cairo_short_string_to_felt;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::JsonRpcClient;
use starknet::signers::LocalWallet;

use crate::auth::{self, ModelContract, OwnerResource, ResourceType};
use crate::{execute, migration};

const ACTION_CONTRACT_NAME: &str = "dojo_examples::actions::actions";

/// Setups the project by migrating the spawn-and-moves project.
///
/// # Returns
///
/// A [`WorldContract`] initialized with the migrator account,
/// the account 0 of the sequencer.
async fn setup(
    sequencer: &TestSequencer,
) -> Result<WorldContract<SingleOwnerAccount<JsonRpcClient<HttpTransport>, LocalWallet>>> {
    let config = build_test_config("../../../examples/spawn-and-move/Scarb.toml")?;
    let ws = ops::read_workspace(config.manifest_path(), &config)
        .unwrap_or_else(|op| panic!("Error building workspace: {op:?}"));
    let base_dir = "../../../examples/spawn-and-move";
    let target_dir = format!("{}/target/dev", base_dir);

    let migration = prepare_migration(base_dir.into(), target_dir.into())?;

    let mut account = sequencer.account();
    account.set_block_id(BlockId::Tag(BlockTag::Pending));

    let output = migration::execute_strategy(
        &ws,
        &migration,
        &account,
        Some(TxConfig { wait: true, ..Default::default() }),
    )
    .await?;
    let world = WorldContract::new(output.world_address, account);

    Ok(world)
}

#[tokio::test(flavor = "multi_thread")]
async fn auth_grant_writer_ok() {
    let sequencer =
        TestSequencer::start(SequencerConfig::default(), get_default_test_starknet_config()).await;

    let world = setup(&sequencer).await.unwrap();

    // Shouldn't have any permission at this point.
    let account2 = sequencer.account_at_index(2);

    // Setup new world contract handler with account 2.
    let world_2 = WorldContract::new(world.address, account2);

    assert!(!execute_spawn(&world_2).await);

    // Account2 does not have the permission to write, but granting
    // writer to the actions contract allows the execution of it's systems by
    // any account.
    let moves_mc = ModelContract {
        model: cairo_short_string_to_felt("Moves").unwrap(),
        contract: ACTION_CONTRACT_NAME.to_string(),
    };

    let position_mc = ModelContract {
        model: cairo_short_string_to_felt("Position").unwrap(),
        contract: ACTION_CONTRACT_NAME.to_string(),
    };

    auth::grant_writer(
        &world,
        vec![moves_mc, position_mc],
        TxConfig { wait: true, ..Default::default() },
    )
    .await
    .unwrap();

    assert!(execute_spawn(&world_2).await);
}

#[tokio::test(flavor = "multi_thread")]
async fn auth_grant_owner_ok() {
    let sequencer =
        TestSequencer::start(SequencerConfig::default(), get_default_test_starknet_config()).await;

    let world = setup(&sequencer).await.unwrap();

    // Shouldn't have any permission at this point.
    let account_2 = sequencer.account_at_index(2);
    let account_2_addr = account_2.address();

    // Setup new world contract handler with account 2.
    let world_2 = WorldContract::new(world.address, account_2);

    assert!(!execute_spawn(&world_2).await);

    // Account2 does not have the permission to write, let's give this account
    // ownership of both models.
    let moves = OwnerResource {
        resource: ResourceType::Model(cairo_short_string_to_felt("Moves").unwrap()),
        owner: account_2_addr,
    };

    let position = OwnerResource {
        resource: ResourceType::Model(cairo_short_string_to_felt("Position").unwrap()),
        owner: account_2_addr,
    };

    auth::grant_owner(&world, vec![moves, position], TxConfig { wait: true, ..Default::default() })
        .await
        .unwrap();

    assert!(execute_spawn(&world_2).await);
}

/// Executes the `spawn` system on `actions` contract.
///
/// # Returns
///
/// True if the execution was successful, false otherwise.
async fn execute_spawn<A: ConnectedAccount + Sync + Send + 'static>(
    world: &WorldContract<A>,
) -> bool {
    let contract_actions = ACTION_CONTRACT_NAME.to_string();
    let system_spawn = "spawn".to_string();

    execute::execute(
        contract_actions,
        system_spawn,
        vec![],
        world,
        TxConfig { wait: true, ..Default::default() },
    )
    .await
    .is_ok()
}
