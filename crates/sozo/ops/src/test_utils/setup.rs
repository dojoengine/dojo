use anyhow::Result;
use dojo_test_utils::compiler::CompilerTestSetup;
use dojo_test_utils::migration::prepare_migration_with_world_and_seed;
use dojo_world::contracts::world::WorldContract;
use dojo_world::metadata::get_default_namespace_from_ws;
use dojo_world::migration::strategy::MigrationStrategy;
use dojo_world::migration::world::WorldDiff;
use dojo_world::migration::TxnConfig;
use katana_runner::KatanaRunner;
use scarb::compiler::Profile;
use scarb::core::{Config, Workspace};
use scarb::ops;
use starknet::accounts::SingleOwnerAccount;
use starknet::core::types::{BlockId, BlockTag};
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::JsonRpcClient;
use starknet::signers::LocalWallet;

use crate::migration;

/// Load the spawn-and-moves project configuration from a copy of the project
/// into a temporary directory to avoid any race during multithreading testing.
///
/// We may in the future add locking mechanism to ensure sozo locks each file
/// being read, to avoid race conditions.
///
/// # Returns
///
/// A [`Config`] object loaded from the spawn-and-moves Scarb.toml file.
pub fn load_config() -> Config {
    let setup = CompilerTestSetup::from_examples("../../dojo-core", "../../../examples/");
    setup.build_test_config("spawn-and-move", Profile::DEV)
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
pub fn setup_migration(config: &Config, seed: &str) -> Result<(MigrationStrategy, WorldDiff)> {
    let ws = setup_ws(config);

    let manifest_path = config.manifest_path();
    let base_dir = manifest_path.parent().unwrap();
    let target_dir = format!("{}/target/dev", base_dir);

    let default_namespace = get_default_namespace_from_ws(&ws).unwrap();

    prepare_migration_with_world_and_seed(
        base_dir.into(),
        target_dir.into(),
        None,
        seed,
        &default_namespace,
    )
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
    sequencer: &KatanaRunner,
) -> Result<WorldContract<SingleOwnerAccount<JsonRpcClient<HttpTransport>, LocalWallet>>> {
    let config = load_config();
    let ws = setup_ws(&config);
    let ui = config.ui();

    let (migration, diff) = setup_migration(&config, "dojo_examples")?;
    let default_namespace = get_default_namespace_from_ws(&ws).unwrap();

    let mut account = sequencer.account(0);
    account.set_block_id(BlockId::Tag(BlockTag::Pending));

    let output = migration::execute_strategy(
        &ws,
        &migration,
        &account,
        TxnConfig { wait: true, ..Default::default() },
    )
    .await?;

    let world = WorldContract::new(output.world_address, account)
        .with_block(BlockId::Tag(BlockTag::Pending));

    let (grant, revoke) =
        migration::find_authorization_diff(&ui, &world, &diff, Some(&output), &default_namespace)
            .await?;

    migration::auto_authorize(
        &ws,
        &world,
        &TxnConfig { wait: true, ..Default::default() },
        &default_namespace,
        &grant,
        &revoke,
    )
    .await?;

    Ok(world)
}

/// Setups the project from an runner starting with an existing world.
///
/// # Arguments
///
/// * `sequencer` - The sequencer used for tests.
///
/// # Returns
///
/// A [`WorldContract`] initialized with the migrator account,
/// the account 0 of the sequencer.
pub async fn setup_with_world(
    sequencer: &KatanaRunner,
) -> Result<WorldContract<SingleOwnerAccount<JsonRpcClient<HttpTransport>, LocalWallet>>> {
    let config = load_config();

    let (migration, _) = setup_migration(&config, "dojo_examples")?;

    let mut account = sequencer.account(0);
    account.set_block_id(BlockId::Tag(BlockTag::Pending));

    let world = WorldContract::new(migration.world_address, account)
        .with_block(BlockId::Tag(BlockTag::Pending));

    Ok(world)
}
