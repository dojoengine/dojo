use std::str::FromStr;

use camino::Utf8PathBuf;
use dojo_test_utils::compiler;
use dojo_test_utils::migration::prepare_migration;
use dojo_world::contracts::world::WorldContractReader;
use dojo_world::metadata::dojo_metadata_from_workspace;
use dojo_world::migration::TxnConfig;
use dojo_world::utils::{TransactionExt, TransactionWaiter};
use katana_runner::KatanaRunner;
use scarb::ops;
use sozo_ops::migration::execute_strategy;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use starknet::accounts::{Account, Call};
use starknet::core::types::{BlockId, BlockTag};
use starknet::core::utils::get_selector_from_name;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::{JsonRpcClient, Provider};
use starknet_crypto::{poseidon_hash_many, FieldElement};
use tokio::sync::broadcast;

use crate::engine::{Engine, EngineConfig, Processors};
use crate::processors::register_model::RegisterModelProcessor;
use crate::processors::store_del_record::StoreDelRecordProcessor;
use crate::processors::store_set_record::StoreSetRecordProcessor;
use crate::sql::Sql;

pub async fn bootstrap_engine<P>(
    world: WorldContractReader<P>,
    db: Sql,
    provider: P,
) -> Result<Engine<P>, Box<dyn std::error::Error>>
where
    P: Provider + Send + Sync,
{
    let (shutdown_tx, _) = broadcast::channel(1);
    let mut engine = Engine::new(
        world,
        db,
        provider,
        Processors {
            event: vec![
                Box::new(RegisterModelProcessor),
                Box::new(StoreSetRecordProcessor),
                Box::new(StoreDelRecordProcessor),
            ],
            ..Processors::default()
        },
        EngineConfig::default(),
        shutdown_tx,
        None,
    );

    let _ = engine.sync_to_head(0, None).await?;

    Ok(engine)
}

#[tokio::test(flavor = "multi_thread")]
async fn test_load_from_remote() {
    let options =
        SqliteConnectOptions::from_str("sqlite::memory:").unwrap().create_if_missing(true);
    let pool = SqlitePoolOptions::new().max_connections(5).connect_with(options).await.unwrap();
    sqlx::migrate!("../migrations").run(&pool).await.unwrap();

    let source_project_dir = Utf8PathBuf::from("../../../examples/spawn-and-move/");
    let dojo_core_path = Utf8PathBuf::from("../../dojo-core");

    let config = compiler::copy_tmp_config(&source_project_dir, &dojo_core_path);
    let ws = scarb::ops::read_workspace(config.manifest_path(), &config).unwrap();
    let dojo_metadata =
        dojo_metadata_from_workspace(&ws).expect("No current package with dojo metadata found.");

    let manifest_path = config.manifest_path();
    let base_dir = manifest_path.parent().unwrap();
    let target_dir = format!("{}/target/dev", base_dir);

    let mut migration =
        prepare_migration(base_dir.into(), target_dir.into(), dojo_metadata.skip_migration)
            .unwrap();
    migration.resolve_variable(migration.world_address().unwrap()).unwrap();

    let sequencer = KatanaRunner::new().expect("Failed to start runner.");

    let provider = JsonRpcClient::new(HttpTransport::new(sequencer.url()));

    let world = WorldContractReader::new(migration.world_address().unwrap(), &provider);

    let mut account = sequencer.account(0);
    account.set_block_id(BlockId::Tag(BlockTag::Pending));

    let ws = ops::read_workspace(config.manifest_path(), &config)
        .unwrap_or_else(|op| panic!("Error building workspace: {op:?}"));

    let migration_output =
        execute_strategy(&ws, &migration, &account, TxnConfig::init_wait()).await.unwrap();

    let world_address = migration_output.world_address;

    assert!(migration.world_address().unwrap() == world_address);

    // spawn
    let tx = account
        .execute(vec![Call {
            to: migration_output
                .contracts
                .first()
                .expect("shouldn't be empty")
                .as_ref()
                .expect("should be deployed")
                .contract_address,
            selector: get_selector_from_name("spawn").unwrap(),
            calldata: vec![],
        }])
        .send()
        .await
        .unwrap();

    TransactionWaiter::new(tx.transaction_hash, &provider).await.unwrap();

    let mut db = Sql::new(pool.clone(), world_address).await.unwrap();
    let _ = bootstrap_engine(world, db.clone(), &provider).await;

    let _block_timestamp = 1710754478_u64;
    let models = sqlx::query("SELECT * FROM models").fetch_all(&pool).await.unwrap();
    assert_eq!(models.len(), 8);

    let (id, name, packed_size, unpacked_size): (String, String, u8, u8) = sqlx::query_as(
        "SELECT id, name, packed_size, unpacked_size FROM models WHERE name = 'Position'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(id, format!("{:#x}", get_selector_from_name("Position").unwrap()));
    assert_eq!(name, "Position");
    assert_eq!(packed_size, 1);
    assert_eq!(unpacked_size, 2);

    let (id, name, packed_size, unpacked_size): (String, String, u8, u8) = sqlx::query_as(
        "SELECT id, name, packed_size, unpacked_size FROM models WHERE name = 'Moves'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(id, format!("{:#x}", get_selector_from_name("Moves").unwrap()));
    assert_eq!(name, "Moves");
    assert_eq!(packed_size, 0);
    assert_eq!(unpacked_size, 2);

    let (id, name, packed_size, unpacked_size): (String, String, u8, u8) = sqlx::query_as(
        "SELECT id, name, packed_size, unpacked_size FROM models WHERE name = 'PlayerConfig'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(id, format!("{:#x}", get_selector_from_name("PlayerConfig").unwrap()));
    assert_eq!(name, "PlayerConfig");
    assert_eq!(packed_size, 0);
    assert_eq!(unpacked_size, 0);

    assert_eq!(count_table("entities", &pool).await, 2);

    let (id, keys): (String, String) = sqlx::query_as(
        format!(
            "SELECT id, keys FROM entities WHERE id = '{:#x}'",
            poseidon_hash_many(&[account.address()])
        )
        .as_str(),
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(id, format!("{:#x}", poseidon_hash_many(&[account.address()])));
    assert_eq!(keys, format!("{:#x}/", account.address()));

    db.execute().await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn test_load_from_remote_del() {
    let options =
        SqliteConnectOptions::from_str("sqlite::memory:").unwrap().create_if_missing(true);
    let pool = SqlitePoolOptions::new().max_connections(5).connect_with(options).await.unwrap();
    sqlx::migrate!("../migrations").run(&pool).await.unwrap();

    let source_project_dir = Utf8PathBuf::from("../../../examples/spawn-and-move/");
    let dojo_core_path = Utf8PathBuf::from("../../dojo-core");

    let config = compiler::copy_tmp_config(&source_project_dir, &dojo_core_path);
    let ws = scarb::ops::read_workspace(config.manifest_path(), &config).unwrap();
    let dojo_metadata =
        dojo_metadata_from_workspace(&ws).expect("No current package with dojo metadata found.");

    let manifest_path = config.manifest_path();
    let base_dir = manifest_path.parent().unwrap();
    let target_dir = format!("{}/target/dev", base_dir);

    let mut migration =
        prepare_migration(base_dir.into(), target_dir.into(), dojo_metadata.skip_migration)
            .unwrap();
    migration.resolve_variable(migration.world_address().unwrap()).unwrap();

    let sequencer = KatanaRunner::new().expect("Failed to start runner.");

    let provider = JsonRpcClient::new(HttpTransport::new(sequencer.url()));

    let world = WorldContractReader::new(migration.world_address().unwrap(), &provider);

    let mut account = sequencer.account(0);
    account.set_block_id(BlockId::Tag(BlockTag::Pending));

    let ws = ops::read_workspace(config.manifest_path(), &config)
        .unwrap_or_else(|op| panic!("Error building workspace: {op:?}"));

    let migration_output =
        execute_strategy(&ws, &migration, &account, TxnConfig::init_wait()).await.unwrap();

    let world_address = migration_output.world_address;

    assert!(migration.world_address().unwrap() == world_address);

    // spawn
    account
        .execute(vec![Call {
            to: migration_output
                .contracts
                .first()
                .expect("shouldn't be empty")
                .as_ref()
                .expect("should be deployed")
                .contract_address,
            selector: get_selector_from_name("spawn").unwrap(),
            calldata: vec![],
        }])
        .send_with_cfg(&TxnConfig::init_wait())
        .await
        .unwrap();

    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    // Set player config.
    account
        .execute(vec![Call {
            to: migration_output
                .contracts
                .first()
                .expect("shouldn't be empty")
                .as_ref()
                .expect("should be deployed")
                .contract_address,
            selector: get_selector_from_name("set_player_config").unwrap(),
            // Empty ByteArray.
            calldata: vec![FieldElement::ZERO, FieldElement::ZERO, FieldElement::ZERO],
        }])
        .send_with_cfg(&TxnConfig::init_wait())
        .await
        .unwrap();

    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    account
        .execute(vec![Call {
            to: migration_output
                .contracts
                .first()
                .expect("shouldn't be empty")
                .as_ref()
                .expect("should be deployed")
                .contract_address,
            selector: get_selector_from_name("reset_player_config").unwrap(),
            calldata: vec![],
        }])
        .send_with_cfg(&TxnConfig::init_wait())
        .await
        .unwrap();

    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    let mut db = Sql::new(pool.clone(), world_address).await.unwrap();
    let _ = bootstrap_engine(world, db.clone(), &provider).await;

    assert_eq!(count_table("PlayerConfig", &pool).await, 0);
    assert_eq!(count_table("PlayerConfig$favorite_item", &pool).await, 0);
    assert_eq!(count_table("PlayerConfig$items", &pool).await, 0);

    // TODO: check how we can have a test that is more chronological with Torii re-syncing
    // to ensure we can test intermediate states.

    db.execute().await.unwrap();
}

/// Count the number of rows in a table.
///
/// # Arguments
/// * `table_name` - The name of the table to count the rows of.
/// * `pool` - The database pool.
///
/// # Returns
/// The number of rows in the table.
async fn count_table(table_name: &str, pool: &sqlx::Pool<sqlx::Sqlite>) -> i64 {
    let count_query = format!("SELECT COUNT(*) FROM {}", table_name);
    let count: (i64,) = sqlx::query_as(&count_query).fetch_one(pool).await.unwrap();

    count.0
}
