use anyhow::Result;
use dojo_test_utils::migration::prepare_migration_with_world_and_seed;
use dojo_test_utils::setup::TestSetup;
use dojo_utils::TxnConfig;
use dojo_world::contracts::world::WorldContract;
use dojo_world::metadata::get_default_namespace_from_ws;
use dojo_world::migration::strategy::MigrationStrategy;
use dojo_world::migration::world::WorldDiff;
use katana_runner::KatanaRunner;
use scarb_interop::Profile;
use starknet::accounts::{ExecutionEncoding, SingleOwnerAccount};
use starknet::core::types::{BlockId, BlockTag};
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::{AnyProvider, JsonRpcClient, Provider};
use starknet::signers::{LocalWallet, SigningKey};

use crate::migration;

/// Get the declarers from the sequencer.
pub async fn get_declarers_from_sequencer(
    sequencer: &KatanaRunner,
) -> Vec<SingleOwnerAccount<AnyProvider, LocalWallet>> {
    let chain_id = sequencer.provider().chain_id().await.unwrap();

    let mut accounts = vec![];
    for a in sequencer.accounts_data() {
        let provider =
            AnyProvider::JsonRpcHttp(JsonRpcClient::new(HttpTransport::new(sequencer.url())));

        let signer = LocalWallet::from(SigningKey::from_secret_scalar(
            a.private_key.as_ref().unwrap().secret_scalar(),
        ));

        let account =
            SingleOwnerAccount::new(provider, signer, a.address, chain_id, ExecutionEncoding::New);

        accounts.push(account);
    }

    accounts
}

/// Load the spawn-and-moves project configuration from a copy of the project
/// into a temporary directory to avoid any race during multithreading testing.
///
/// We may in the future add locking mechanism to ensure sozo locks each file
/// being read, to avoid race conditions.
///
/// # Returns
///
/// A [`Config`] object loaded from the spawn-and-moves Scarb.toml file.
pub fn load_metadata() -> Metadata {
    let setup = TestSetup::from_examples("../../dojo/core", "../../../examples/");
    setup.load_metadata("spawn-and-move", Profile::DEV)
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
    let metadata = load_metadata();
    let ws = setup_ws(&config);
    let ui = config.ui();

    let (migration, diff) = setup_migration(&metadata, "dojo_examples")?;
    let default_namespace = get_default_namespace_from_ws(&ws).unwrap();

    let mut account = sequencer.account(0);
    account.set_block_id(BlockId::Tag(BlockTag::Pending));

    let chain_id = sequencer.provider().chain_id().await.unwrap();

    let mut accounts = vec![];
    for a in sequencer.accounts_data() {
        let provider =
            AnyProvider::JsonRpcHttp(JsonRpcClient::new(HttpTransport::new(sequencer.url())));

        let signer = LocalWallet::from(SigningKey::from_secret_scalar(
            a.private_key.as_ref().unwrap().secret_scalar(),
        ));

        let account =
            SingleOwnerAccount::new(provider, signer, a.address, chain_id, ExecutionEncoding::New);

        accounts.push(account);
    }

    let output = migration::execute_strategy(
        &ws,
        &migration,
        &account,
        TxnConfig { wait: true, ..Default::default() },
        &accounts,
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
        #[cfg(feature = "walnut")]
        &None,
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
