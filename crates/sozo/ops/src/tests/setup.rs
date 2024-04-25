use anyhow::Result;
use dojo_test_utils::compiler::build_test_config;
use dojo_test_utils::migration::prepare_migration_with_world_and_seed;
use dojo_test_utils::sequencer::TestSequencer;
use dojo_world::contracts::world::WorldContract;
use dojo_world::migration::strategy::MigrationStrategy;
use dojo_world::migration::TxnConfig;
use scarb::core::{Config, Workspace};
use scarb::ops;
use starknet::accounts::SingleOwnerAccount;
use starknet::core::types::{BlockId, BlockTag};
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::JsonRpcClient;
use starknet::signers::LocalWallet;

use crate::migration;

/// Load the spawn-and-moves project configuration.
///
/// # Returns
///
/// A [`Config`] object loaded from the spawn-and-moves Scarb.toml file.
pub fn load_config() -> Config {
    build_test_config("../../../examples/spawn-and-move/Scarb.toml")
        .unwrap_or_else(|c| panic!("Error loading config: {c:?}"))
}

/// Setups the workspace for the spawn-and-moves project.
///
/// # Arguments
/// * `config` - the project configuration.
///
/// # Returns
///
/// A [`Workspace`] loaded from the spawn-and-moves project.
pub fn setup_ws(config: &Config) -> Workspace<'_> {
    ops::read_workspace(config.manifest_path(), config)
        .unwrap_or_else(|op| panic!("Error building workspace: {op:?}"))
}

/// Prepare the migration for the spawn-and-moves project.
///
/// # Returns
///
/// A [`MigrationStrategy`] to execute to migrate the full spawn-and-moves project.
pub fn setup_migration() -> Result<MigrationStrategy> {
    let base_dir = "../../../examples/spawn-and-move";
    let target_dir = format!("{}/target/dev", base_dir);

    prepare_migration_with_world_and_seed(base_dir.into(), target_dir.into(), None, "sozo_test")
}

/// Setups the project by migrating the full spawn-and-moves project.
///
/// # Arguments
///
/// * `sequencer` - The sequencer used for tests.
///
/// # Returns
///
/// A [`WorldContract`] initialized with the migrator account,
/// the account 0 of the sequencer.
pub async fn setup(
    sequencer: &TestSequencer,
) -> Result<WorldContract<SingleOwnerAccount<JsonRpcClient<HttpTransport>, LocalWallet>>> {
    let config = load_config();
    let ws = setup_ws(&config);

    let migration = setup_migration()?;

    let mut account = sequencer.account();
    account.set_block_id(BlockId::Tag(BlockTag::Pending));

    let output = migration::execute_strategy(
        &ws,
        &migration,
        &account,
        TxnConfig { wait: true, ..Default::default() },
    )
    .await?;
    let world = WorldContract::new(output.world_address, account);

    Ok(world)
}
