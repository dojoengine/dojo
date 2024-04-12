use anyhow::Result;
use dojo_test_utils::compiler::build_test_config;
use dojo_test_utils::migration::prepare_migration;
use dojo_test_utils::sequencer::TestSequencer;
use dojo_world::contracts::world::WorldContract;
use dojo_world::migration::TxnConfig;
use scarb::ops;
use starknet::accounts::SingleOwnerAccount;
use starknet::core::types::{BlockId, BlockTag};
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::JsonRpcClient;
use starknet::signers::LocalWallet;

use crate::migration;

/// Setups the project by migrating the full spawn-and-moves project.
///
/// # Returns
///
/// A [`WorldContract`] initialized with the migrator account,
/// the account 0 of the sequencer.
pub async fn setup(
    sequencer: &TestSequencer,
) -> Result<WorldContract<SingleOwnerAccount<JsonRpcClient<HttpTransport>, LocalWallet>>> {
    let config = build_test_config("../../../examples/spawn-and-move/Scarb.toml")?;
    let ws = ops::read_workspace(config.manifest_path(), &config)
        .unwrap_or_else(|op| panic!("Error building workspace: {op:?}"));
    let base_dir = "../../../examples/spawn-and-move";
    let target_dir = format!("{}/target/dev", base_dir);

    let mut migration = prepare_migration(base_dir.into(), target_dir.into())?;

    let mut account = sequencer.account();
    account.set_block_id(BlockId::Tag(BlockTag::Pending));

    let output = migration::execute_strategy(
        &ws,
        &mut migration,
        &account,
        Some(TxnConfig { wait: true, ..Default::default() }),
    )
    .await?;
    let world = WorldContract::new(output.world_address, account);

    Ok(world)
}
