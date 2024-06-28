use anyhow::Result;
use camino::Utf8PathBuf;
use dojo_test_utils::compiler;
use dojo_test_utils::migration::prepare_migration_with_world_and_seed;
use dojo_world::contracts::world::WorldContract;
use dojo_world::manifest::utils::get_default_namespace_from_ws;
use dojo_world::migration::strategy::MigrationStrategy;
use dojo_world::migration::TxnConfig;
use katana_runner::KatanaRunner;
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
    // To avoid race conditions with other tests, all the project files
    // are copied to ensure safe parallel execution.
    let source_project_dir = Utf8PathBuf::from("../../../examples/spawn-and-move/");
    let dojo_core_path = Utf8PathBuf::from("../../dojo-core");

    compiler::copy_tmp_config(&source_project_dir, &dojo_core_path)
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
pub fn setup_migration(config: &Config) -> Result<MigrationStrategy> {
    let ws = setup_ws(config);

    let manifest_path = config.manifest_path();
    let base_dir = manifest_path.parent().unwrap();
    let target_dir = format!("{}/target/dev", base_dir);

    let default_namespace = get_default_namespace_from_ws(&ws);

    prepare_migration_with_world_and_seed(
        base_dir.into(),
        target_dir.into(),
        None,
        "sozo_test",
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

    let migration = setup_migration(&config)?;

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

    Ok(world)
}
