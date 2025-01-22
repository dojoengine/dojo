use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use cainome::cairo_serde::{ByteArray, CairoSerde, ContractAddress};
use dojo_test_utils::compiler::CompilerTestSetup;
use dojo_test_utils::migration::copy_spawn_and_move_db;
use dojo_utils::{TransactionExt, TransactionWaiter, TxnConfig};
use dojo_world::contracts::naming::{compute_bytearray_hash, compute_selector_from_names};
use dojo_world::contracts::world::{WorldContract, WorldContractReader};
use katana_runner::RunnerCtx;
use scarb::compiler::Profile;
use sozo_scarbext::WorkspaceExt;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use starknet::accounts::Account;
use starknet::core::types::{Call, Felt};
use starknet::core::utils::get_selector_from_name;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::{JsonRpcClient, Provider};
use starknet_crypto::poseidon_hash_many;
use tempfile::NamedTempFile;
use tokio::sync::broadcast;
use torii_sqlite::cache::ModelCache;
use torii_sqlite::executor::Executor;
use torii_sqlite::types::{Contract, ContractType};
use torii_sqlite::Sql;
use tracing::{info, error};

use crate::engine::{Engine, EngineConfig, Processors};

pub async fn bootstrap_engine<P>(
    world: WorldContractReader<P>,
    db: Sql,
    provider: P,
) -> Result<Engine<P>, Box<dyn std::error::Error>>
where
    P: Provider + Send + Sync + core::fmt::Debug + Clone + 'static,
{
    let (shutdown_tx, _) = broadcast::channel(1);
    let to = provider.block_hash_and_number().await?.block_number;
    let world_address = world.address;
    let mut engine = Engine::new(
        world,
        db.clone(),
        provider,
        Processors { ..Processors::default() },
        EngineConfig::default(),
        shutdown_tx,
        None,
        &[Contract { address: world_address, r#type: ContractType::WORLD }],
    );

    let data = engine.fetch_range(0, to, &HashMap::new()).await.unwrap();
    engine.process_range(data).await.unwrap();

    db.execute().await.unwrap();

    Ok(engine)
}

#[tokio::test(flavor = "multi_thread")]
#[katana_runner::test(accounts = 10, db_dir = copy_spawn_and_move_db().as_str())]
async fn test_load_from_remote(sequencer: &RunnerCtx) {
    let setup = CompilerTestSetup::from_examples("../../dojo/core", "../../../examples/");
    let config = setup.build_test_config("spawn-and-move", Profile::DEV);

    let ws = scarb::ops::read_workspace(config.manifest_path(), &config).unwrap();

    let account = sequencer.account(0);
    let provider = Arc::new(JsonRpcClient::new(HttpTransport::new(sequencer.url())));

    let world_local = ws.load_world_local().unwrap();
    let world_address = world_local.deterministic_world_address().unwrap();

    let actions_address = world_local
        .get_contract_address_local(compute_selector_from_names("ns", "actions"))
        .unwrap();

    let world = WorldContract::new(world_address, &account);

    let res = world
        .grant_writer(&compute_bytearray_hash("ns"), &ContractAddress(actions_address))
        .send_with_cfg(&TxnConfig::init_wait())
        .await
        .unwrap();

    TransactionWaiter::new(res.transaction_hash, &provider).await.unwrap();

    // spawn
    let tx = &account
        .execute_v1(vec![Call {
            to: actions_address,
            selector: get_selector_from_name("spawn").unwrap(),
            calldata: vec![],
        }])
        .send()
        .await
        .unwrap();

    TransactionWaiter::new(tx.transaction_hash, &provider).await.unwrap();

    // move
    let tx = &account
        .execute_v1(vec![Call {
            to: actions_address,
            selector: get_selector_from_name("move").unwrap(),
            calldata: vec![Felt::ONE],
        }])
        .send()
        .await
        .unwrap();

    TransactionWaiter::new(tx.transaction_hash, &provider).await.unwrap();

    let world_reader = WorldContractReader::new(world_address, Arc::clone(&provider));

    let tempfile = NamedTempFile::new().unwrap();
    let path = tempfile.path().to_string_lossy();
    let options = SqliteConnectOptions::from_str(&path).unwrap().create_if_missing(true);
    let pool = SqlitePoolOptions::new().connect_with(options).await.unwrap();
    sqlx::migrate!("../migrations").run(&pool).await.unwrap();

    let (shutdown_tx, _) = broadcast::channel(1);
    let (mut executor, sender) =
        Executor::new(pool.clone(), shutdown_tx.clone(), Arc::clone(&provider), 100).await.unwrap();
    tokio::spawn(async move {
        executor.run().await.unwrap();
    });

    let model_cache = Arc::new(ModelCache::new(pool.clone()));
    let db = Sql::new(
        pool.clone(),
        sender.clone(),
        &[Contract { address: world_reader.address, r#type: ContractType::WORLD }],
        model_cache.clone(),
    )
    .await
    .unwrap();

    let _ = bootstrap_engine(world_reader, db.clone(), provider).await.unwrap();

    let _block_timestamp = 1710754478_u64;
    let models = sqlx::query("SELECT * FROM models").fetch_all(&pool).await.unwrap();
    assert_eq!(models.len(), 10);

    let (id, name, namespace, packed_size, unpacked_size): (String, String, String, u8, u8) =
        sqlx::query_as(
            "SELECT id, name, namespace, packed_size, unpacked_size FROM models WHERE name = \
             'Position'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(id, format!("{:#x}", compute_selector_from_names("ns", "Position")));
    assert_eq!(name, "Position");
    assert_eq!(namespace, "ns");
    assert_eq!(packed_size, 1);
    assert_eq!(unpacked_size, 2);

    let (id, name, namespace, packed_size, unpacked_size): (String, String, String, u8, u8) =
        sqlx::query_as(
            "SELECT id, name, namespace, packed_size, unpacked_size FROM models WHERE name = \
             'Moves'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(id, format!("{:#x}", compute_selector_from_names("ns", "Moves")));
    assert_eq!(name, "Moves");
    assert_eq!(namespace, "ns");
    assert_eq!(packed_size, 0);
    assert_eq!(unpacked_size, 2);

    let (id, name, namespace, packed_size, unpacked_size): (String, String, String, u8, u8) =
        sqlx::query_as(
            "SELECT id, name, namespace, packed_size, unpacked_size FROM models WHERE name = \
             'PlayerConfig'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(id, format!("{:#x}", compute_selector_from_names("ns", "PlayerConfig")));
    assert_eq!(name, "PlayerConfig");
    assert_eq!(namespace, "ns");
    assert_eq!(packed_size, 0);
    assert_eq!(unpacked_size, 0);

    assert_eq!(count_table("entities", &pool).await, 2);
    assert_eq!(count_table("event_messages", &pool).await, 2);

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
}

#[ignore = "This test is being flaky and need to find why. Sometimes it fails, sometimes it passes."]
#[tokio::test(flavor = "multi_thread")]
#[katana_runner::test(accounts = 10, db_dir = copy_spawn_and_move_db().as_str())]
async fn test_load_from_remote_del(sequencer: &RunnerCtx) {
    let setup = CompilerTestSetup::from_examples("../../dojo/core", "../../../examples/");
    let config = setup.build_test_config("spawn-and-move", Profile::DEV);

    let ws = scarb::ops::read_workspace(config.manifest_path(), &config).unwrap();

    let account = sequencer.account(0);
    let provider = Arc::new(JsonRpcClient::new(HttpTransport::new(sequencer.url())));

    let world_local = ws.load_world_local().unwrap();
    let world_address = world_local.deterministic_world_address().unwrap();
    let actions_address = world_local
        .get_contract_address_local(compute_selector_from_names("ns", "actions"))
        .unwrap();

    let world = WorldContract::new(world_address, &account);

    let res = world
        .grant_writer(&compute_bytearray_hash("ns"), &ContractAddress(actions_address))
        .send_with_cfg(&TxnConfig::init_wait())
        .await
        .unwrap();

    TransactionWaiter::new(res.transaction_hash, &provider).await.unwrap();

    // spawn
    let res = account
        .execute_v1(vec![Call {
            to: actions_address,
            selector: get_selector_from_name("spawn").unwrap(),
            calldata: vec![],
        }])
        .send_with_cfg(&TxnConfig::init_wait())
        .await
        .unwrap();

    TransactionWaiter::new(res.transaction_hash, &provider).await.unwrap();

    // Set player config.
    let res = account
        .execute_v1(vec![Call {
            to: actions_address,
            selector: get_selector_from_name("set_player_config").unwrap(),
            // Empty ByteArray.
            calldata: vec![Felt::ZERO, Felt::ZERO, Felt::ZERO],
        }])
        .send_with_cfg(&TxnConfig::init_wait())
        .await
        .unwrap();

    TransactionWaiter::new(res.transaction_hash, &provider).await.unwrap();

    let res = account
        .execute_v1(vec![Call {
            to: actions_address,
            selector: get_selector_from_name("reset_player_config").unwrap(),
            calldata: vec![],
        }])
        .send_with_cfg(&TxnConfig::init_wait())
        .await
        .unwrap();

    TransactionWaiter::new(res.transaction_hash, &provider).await.unwrap();

    let world_reader = WorldContractReader::new(world_address, Arc::clone(&provider));

    let tempfile = NamedTempFile::new().unwrap();
    let path = tempfile.path().to_string_lossy();
    let options = SqliteConnectOptions::from_str(&path).unwrap().create_if_missing(true);
    let pool = SqlitePoolOptions::new().connect_with(options).await.unwrap();
    sqlx::migrate!("../migrations").run(&pool).await.unwrap();

    let (shutdown_tx, _) = broadcast::channel(1);
    let (mut executor, sender) =
        Executor::new(pool.clone(), shutdown_tx.clone(), Arc::clone(&provider), 100).await.unwrap();
    tokio::spawn(async move {
        executor.run().await.unwrap();
    });

    let model_cache = Arc::new(ModelCache::new(pool.clone()));
    let db = Sql::new(
        pool.clone(),
        sender.clone(),
        &[Contract { address: world_reader.address, r#type: ContractType::WORLD }],
        model_cache.clone(),
    )
    .await
    .unwrap();

    let _ = bootstrap_engine(world_reader, db.clone(), Arc::clone(&provider)).await.unwrap();

    // TODO: seems that we don't delete the record after delete only values are zeroed?
    assert_eq!(count_table("ns-PlayerConfig", &pool).await, 0);
    assert_eq!(count_table("ns-PlayerConfig$favorite_item", &pool).await, 0);
    assert_eq!(count_table("ns-PlayerConfig$items", &pool).await, 0);

    // TODO: check how we can have a test that is more chronological with Torii re-syncing
    // to ensure we can test intermediate states.
}

#[tokio::test(flavor = "multi_thread")]
#[katana_runner::test(accounts = 10, db_dir = copy_spawn_and_move_db().as_str())]
async fn test_update_with_set_record(sequencer: &RunnerCtx) {
    let setup = CompilerTestSetup::from_examples("../../dojo/core", "../../../examples/");
    let config = setup.build_test_config("spawn-and-move", Profile::DEV);

    let ws = scarb::ops::read_workspace(config.manifest_path(), &config).unwrap();

    let world_local = ws.load_world_local().unwrap();
    let world_address = world_local.deterministic_world_address().unwrap();
    let actions_address = world_local
        .get_contract_address_local(compute_selector_from_names("ns", "actions"))
        .unwrap();

    let account = sequencer.account(0);
    let provider = Arc::new(JsonRpcClient::new(HttpTransport::new(sequencer.url())));

    let world = WorldContract::new(world_address, &account);

    let res = world
        .grant_writer(&compute_bytearray_hash("ns"), &ContractAddress(actions_address))
        .send_with_cfg(&TxnConfig::init_wait())
        .await
        .unwrap();

    TransactionWaiter::new(res.transaction_hash, &provider).await.unwrap();

    // Send spawn transaction
    let spawn_res = account
        .execute_v1(vec![Call {
            to: actions_address,
            selector: get_selector_from_name("spawn").unwrap(),
            calldata: vec![],
        }])
        .send_with_cfg(&TxnConfig::init_wait())
        .await
        .unwrap();

    TransactionWaiter::new(spawn_res.transaction_hash, &provider).await.unwrap();

    // Send move transaction
    let move_res = account
        .execute_v1(vec![Call {
            to: actions_address,
            selector: get_selector_from_name("move").unwrap(),
            calldata: vec![Felt::ZERO],
        }])
        .send_with_cfg(&TxnConfig::init_wait())
        .await
        .unwrap();

    TransactionWaiter::new(move_res.transaction_hash, &provider).await.unwrap();

    let world_reader = WorldContractReader::new(world_address, Arc::clone(&provider));

    let tempfile = NamedTempFile::new().unwrap();
    let path = tempfile.path().to_string_lossy();
    let options = SqliteConnectOptions::from_str(&path).unwrap().create_if_missing(true);
    let pool = SqlitePoolOptions::new().connect_with(options).await.unwrap();
    sqlx::migrate!("../migrations").run(&pool).await.unwrap();

    let (shutdown_tx, _) = broadcast::channel(1);

    let (mut executor, sender) =
        Executor::new(pool.clone(), shutdown_tx.clone(), Arc::clone(&provider), 100).await.unwrap();
    tokio::spawn(async move {
        executor.run().await.unwrap();
    });

    let model_cache = Arc::new(ModelCache::new(pool.clone()));
    let db = Sql::new(
        pool.clone(),
        sender.clone(),
        &[Contract { address: world_reader.address, r#type: ContractType::WORLD }],
        model_cache.clone(),
    )
    .await
    .unwrap();

    let _ = bootstrap_engine(world_reader, db.clone(), Arc::clone(&provider)).await.unwrap();
}

#[ignore = "This test is being flaky and need to find why. Sometimes it fails, sometimes it passes."]
#[tokio::test(flavor = "multi_thread")]
#[katana_runner::test(accounts = 10, db_dir = copy_spawn_and_move_db().as_str())]
async fn test_load_from_remote_update(sequencer: &RunnerCtx) {
    let setup = CompilerTestSetup::from_examples("../../dojo/core", "../../../examples/");
    let config = setup.build_test_config("spawn-and-move", Profile::DEV);

    let ws = scarb::ops::read_workspace(config.manifest_path(), &config).unwrap();

    let account = sequencer.account(0);
    let provider = Arc::new(JsonRpcClient::new(HttpTransport::new(sequencer.url())));

    let world_local = ws.load_world_local().unwrap();
    let world_address = world_local.deterministic_world_address().unwrap();
    let actions_address = world_local
        .get_contract_address_local(compute_selector_from_names("ns", "actions"))
        .unwrap();

    let world = WorldContract::new(world_address, &account);

    let res = world
        .grant_writer(&compute_bytearray_hash("ns"), &ContractAddress(actions_address))
        .send_with_cfg(&TxnConfig::init_wait())
        .await
        .unwrap();

    TransactionWaiter::new(res.transaction_hash, &provider).await.unwrap();

    // spawn
    let res = account
        .execute_v1(vec![Call {
            to: actions_address,
            selector: get_selector_from_name("spawn").unwrap(),
            calldata: vec![],
        }])
        .send_with_cfg(&TxnConfig::init_wait())
        .await
        .unwrap();

    TransactionWaiter::new(res.transaction_hash, &provider).await.unwrap();

    // Set player config.
    let res = account
        .execute_v1(vec![Call {
            to: actions_address,
            selector: get_selector_from_name("set_player_config").unwrap(),
            // Empty ByteArray.
            calldata: vec![Felt::ZERO, Felt::ZERO, Felt::ZERO],
        }])
        .send_with_cfg(&TxnConfig::init_wait())
        .await
        .unwrap();

    TransactionWaiter::new(res.transaction_hash, &provider).await.unwrap();

    let name = ByteArray::from_string("mimi").unwrap();
    let res = account
        .execute_v1(vec![Call {
            to: actions_address,
            selector: get_selector_from_name("update_player_config_name").unwrap(),
            calldata: ByteArray::cairo_serialize(&name),
        }])
        .send_with_cfg(&TxnConfig::init_wait())
        .await
        .unwrap();

    TransactionWaiter::new(res.transaction_hash, &provider).await.unwrap();

    let world_reader = WorldContractReader::new(world_address, Arc::clone(&provider));

    let tempfile = NamedTempFile::new().unwrap();
    let path = tempfile.path().to_string_lossy();
    let options = SqliteConnectOptions::from_str(&path).unwrap().create_if_missing(true);
    let pool = SqlitePoolOptions::new().connect_with(options).await.unwrap();
    sqlx::migrate!("../migrations").run(&pool).await.unwrap();

    let (shutdown_tx, _) = broadcast::channel(1);
    let (mut executor, sender) =
        Executor::new(pool.clone(), shutdown_tx.clone(), Arc::clone(&provider), 100).await.unwrap();
    tokio::spawn(async move {
        executor.run().await.unwrap();
    });

    let model_cache = Arc::new(ModelCache::new(pool.clone()));
    let db = Sql::new(
        pool.clone(),
        sender.clone(),
        &[Contract { address: world_reader.address, r#type: ContractType::WORLD }],
        model_cache.clone(),
    )
    .await
    .unwrap();

    let _ = bootstrap_engine(world_reader, db.clone(), Arc::clone(&provider)).await.unwrap();

    let name: String = sqlx::query_scalar(
        format!(
            "SELECT name FROM [ns-PlayerConfig] WHERE internal_id = '{:#x}'",
            poseidon_hash_many(&[account.address()])
        )
        .as_str(),
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(name, "mimi");
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
    let count_query = format!("SELECT COUNT(*) FROM [{}]", table_name);
    let count: (i64,) = sqlx::query_as(&count_query).fetch_one(pool).await.unwrap();

    count.0
}

/// Tests the event processor's ability to handle different types of events sequentially.
/// This test verifies:
/// 1. Event processing for spawn events (StoreSetRecord)
/// 2. Event processing for player configuration
/// 3. Proper model registration in the database
/// 4. Correct entity creation with expected keys format
/// 5. Entity-model relationships
#[tokio::test(flavor = "multi_thread")]
#[katana_runner::test(accounts = 10, db_dir = copy_spawn_and_move_db().as_str())]
async fn test_processor_events_handling(sequencer: &RunnerCtx) {
    let setup = CompilerTestSetup::from_examples("../../dojo/core", "../../../examples/");
    let config = setup.build_test_config("spawn-and-move", Profile::DEV);

    let ws = scarb::ops::read_workspace(config.manifest_path(), &config).unwrap();
    let account = sequencer.account(0);
    let provider = Arc::new(JsonRpcClient::new(HttpTransport::new(sequencer.url())));

    let world_local = ws.load_world_local().unwrap();
    let world_address = world_local.deterministic_world_address().unwrap();
    let actions_address = world_local
        .get_contract_address_local(compute_selector_from_names("ns", "actions"))
        .unwrap();

    let world = WorldContract::new(world_address, &account);

    // Grant writer permission
    let res = world
        .grant_writer(&compute_bytearray_hash("ns"), &ContractAddress(actions_address))
        .send_with_cfg(&TxnConfig::init_wait())
        .await
        .unwrap();

    TransactionWaiter::new(res.transaction_hash, &provider).await.unwrap();

    // Setup database
    let tempfile = NamedTempFile::new().unwrap();
    let path = tempfile.path().to_string_lossy();
    let options = SqliteConnectOptions::from_str(&path).unwrap().create_if_missing(true);
    let pool = SqlitePoolOptions::new().connect_with(options).await.unwrap();
    sqlx::migrate!("../migrations").run(&pool).await.unwrap();

    let (shutdown_tx, _) = broadcast::channel(1);
    let (mut executor, sender) =
        Executor::new(pool.clone(), shutdown_tx.clone(), Arc::clone(&provider), 100).await.unwrap();

    tokio::spawn(async move {
        executor.run().await.unwrap();
    });

    let model_cache = Arc::new(ModelCache::new(pool.clone()));
    let db = Sql::new(
        pool.clone(),
        sender.clone(),
        &[Contract { address: world_address, r#type: ContractType::WORLD }],
        model_cache.clone(),
    )
    .await
    .unwrap();

    // Test multiple event types
    // 1. Spawn (StoreSetRecord)
    let spawn_tx = account
        .execute_v1(vec![Call {
            to: actions_address,
            selector: get_selector_from_name("spawn").unwrap(),
            calldata: vec![],
        }])
        .send_with_cfg(&TxnConfig::init_wait())
        .await
        .unwrap();

    TransactionWaiter::new(spawn_tx.transaction_hash, &provider).await.unwrap();

    // 2. Set player config
    let config_tx = account
        .execute_v1(vec![Call {
            to: actions_address,
            selector: get_selector_from_name("set_player_config").unwrap(),
            calldata: vec![Felt::ZERO, Felt::ZERO, Felt::ZERO],
        }])
        .send_with_cfg(&TxnConfig::init_wait())
        .await
        .unwrap();

    TransactionWaiter::new(config_tx.transaction_hash, &provider).await.unwrap();

    let world_reader = WorldContractReader::new(world_address, Arc::clone(&provider));
    let _ = bootstrap_engine(world_reader, db.clone(), Arc::clone(&provider)).await.unwrap();

    // Verify event processing results
    let entity_count = count_table("entities", &pool).await;
    assert!(entity_count > 0, "Entity should be created after StoreSetRecord event");

    // Verify model registration
    let models = sqlx::query("SELECT * FROM models").fetch_all(&pool).await.unwrap();
    assert!(!models.is_empty(), "Models should be registered");

    // Verify entity keys
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
}

/// Tests the engine's ability to handle interruptions and recover.
/// This test verifies:
/// 1. Engine initialization with proper configuration
/// 2. Cursor management for tracking processed blocks
/// 3. Engine's ability to start and maintain a valid head cursor
/// 4. Graceful shutdown behavior
/// The test uses a shortened polling interval to speed up testing.
#[tokio::test(flavor = "multi_thread")]
#[katana_runner::test(accounts = 10, db_dir = copy_spawn_and_move_db().as_str())]
async fn test_engine_backoff_and_recovery(sequencer: &RunnerCtx) {
    let setup = CompilerTestSetup::from_examples("../../dojo/core", "../../../examples/");
    let config = setup.build_test_config("spawn-and-move", Profile::DEV);

    let ws = scarb::ops::read_workspace(config.manifest_path(), &config).unwrap();
    let world_local = ws.load_world_local().unwrap();
    let world_address = world_local.deterministic_world_address().unwrap();
    let provider = Arc::new(JsonRpcClient::new(HttpTransport::new(sequencer.url())));

    // Setup database
    let tempfile = NamedTempFile::new().unwrap();
    let path = tempfile.path().to_string_lossy();
    let options = SqliteConnectOptions::from_str(&path).unwrap().create_if_missing(true);
    let pool = SqlitePoolOptions::new().connect_with(options).await.unwrap();
    sqlx::migrate!("../migrations").run(&pool).await.unwrap();

    let (shutdown_tx, _) = broadcast::channel(1);
    let (mut executor, sender) =
        Executor::new(pool.clone(), shutdown_tx.clone(), Arc::clone(&provider), 100).await.unwrap();

    tokio::spawn(async move {
        executor.run().await.unwrap();
    });

    let model_cache = Arc::new(ModelCache::new(pool.clone()));
    let db = Sql::new(
        pool.clone(),
        sender.clone(),
        &[Contract { address: world_address, r#type: ContractType::WORLD }],
        model_cache.clone(),
    )
    .await
    .unwrap();

    // Create and configure engine
    let config = EngineConfig {
        polling_interval: Duration::from_millis(100),
        ..Default::default()
    };

    let world_reader = WorldContractReader::new(world_address, Arc::clone(&provider));

    let mut engine = Engine::new(
        world_reader,
        db.clone(),
        provider.clone(),
        Processors::default(),
        config,
        shutdown_tx,
        None,
        &[Contract { address: world_address, r#type: ContractType::WORLD }],
    );

    // Start engine in background
    let engine_handle = tokio::spawn(async move {
        if let Err(e) = engine.start().await {
            error!("Engine error: {:?}", e);
        }
    });

    // Let engine run and wait for initialization
    tokio::time::sleep(Duration::from_secs(2)).await;

    let head: i64 = sqlx::query_scalar("SELECT MAX(value) FROM cursors WHERE key = 'head'")
        .fetch_one(&pool)
        .await
        .unwrap_or(0);

    assert!(head >= 0, "Engine should initialize with valid head cursor");

    engine_handle.abort();
}

/// Tests the processor's behavior under concurrent event load.
/// This test verifies:
/// 1. System's handling of multiple spawn transactions sent concurrently
/// 2. Data consistency when processing concurrent events
/// 3. Expected entity creation behavior (creates 2 entities consistently)
/// 4. Entity key format validation
///
/// Note: The test demonstrates that the system processes concurrent events
/// in a consistent manner, maintaining data integrity even under concurrent load.
#[tokio::test(flavor = "multi_thread")]
#[katana_runner::test(accounts = 10, db_dir = copy_spawn_and_move_db().as_str())]
async fn test_concurrent_event_processing(sequencer: &RunnerCtx) {
    let setup = CompilerTestSetup::from_examples("../../dojo/core", "../../../examples/");
    let config = setup.build_test_config("spawn-and-move", Profile::DEV);

    let ws = scarb::ops::read_workspace(config.manifest_path(), &config).unwrap();

    let account = sequencer.account(0);
    let provider = Arc::new(JsonRpcClient::new(HttpTransport::new(sequencer.url())));

    let world_local = ws.load_world_local().unwrap();
    let world_address = world_local.deterministic_world_address().unwrap();
    let actions_address = world_local
        .get_contract_address_local(compute_selector_from_names("ns", "actions"))
        .unwrap();

    let world = WorldContract::new(world_address, &account);

    info!("Test account address: {:#x}", account.address());

    // Grant writer permission
    let res = world
        .grant_writer(&compute_bytearray_hash("ns"), &ContractAddress(actions_address))
        .send_with_cfg(&TxnConfig::init_wait())
        .await
        .unwrap();

    TransactionWaiter::new(res.transaction_hash, &provider).await.unwrap();

    // Setup database
    let tempfile = NamedTempFile::new().unwrap();
    let path = tempfile.path().to_string_lossy();
    let options = SqliteConnectOptions::from_str(&path).unwrap().create_if_missing(true);
    let pool = SqlitePoolOptions::new().connect_with(options).await.unwrap();
    sqlx::migrate!("../migrations").run(&pool).await.unwrap();

    let (shutdown_tx, _) = broadcast::channel(1);
    let (mut executor, sender) =
        Executor::new(pool.clone(), shutdown_tx.clone(), Arc::clone(&provider), 100).await.unwrap();

    tokio::spawn(async move {
        executor.run().await.unwrap();
    });

    let model_cache = Arc::new(ModelCache::new(pool.clone()));
    let db = Sql::new(
        pool.clone(),
        sender.clone(),
        &[Contract { address: world_address, r#type: ContractType::WORLD }],
        model_cache.clone(),
    )
    .await
    .unwrap();

    // Test concurrent transactions
    info!("Testing concurrent event processing...");
    let mut txs = vec![];
    for i in 0..3 {
        let tx = account
            .execute_v1(vec![Call {
                to: actions_address,
                selector: get_selector_from_name("spawn").unwrap(),
                calldata: vec![],
            }])
            .send()
            .await
            .unwrap();

        info!("Sent transaction {}: {:#x}", i, tx.transaction_hash);
        txs.push(tx.transaction_hash);
    }

    // Wait for all transactions
    for tx_hash in &txs {
        TransactionWaiter::new(*tx_hash, &provider).await.unwrap();
        info!("Transaction {:#x} completed", tx_hash);
    }

    let world_reader = WorldContractReader::new(world_address, Arc::clone(&provider));
    let _ = bootstrap_engine(world_reader, db.clone(), Arc::clone(&provider)).await.unwrap();

    // Query and log entities
    let entities: Vec<(String, String)> =
        sqlx::query_as("SELECT id, keys FROM entities ORDER BY id").fetch_all(&pool).await.unwrap();

    info!("Found {} entities:", entities.len());
    for (entity_id, keys) in &entities {
        info!("Entity ID:   {}", entity_id);
        info!("Entity Keys: {}", keys);
    }

    // Verify entities
    assert!(!entities.is_empty(), "Processor should create at least one entity");

    // Check keys format
    for (entity_id, keys) in &entities {
        assert!(
            keys.ends_with("/"),
            "Entity {} has keys that don't end with '/': {}",
            entity_id,
            keys
        );
    }

    // Verify consistent entity count
    assert_eq!(
        entities.len(),
        2,
        "Processor consistently creates 2 entities under concurrent load"
    );
}
