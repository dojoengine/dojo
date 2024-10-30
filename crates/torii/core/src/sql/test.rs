use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

use cainome::cairo_serde::ContractAddress;
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

use crate::engine::{Engine, EngineConfig, Processors};
use crate::executor::Executor;
use crate::sql::Sql;
use crate::types::ContractType;

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
        Arc::new(HashMap::from([(world_address, ContractType::WORLD)])),
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
    let (mut executor, sender) = Executor::new(pool.clone(), shutdown_tx.clone()).await.unwrap();
    tokio::spawn(async move {
        executor.run().await.unwrap();
    });

    let db = Sql::new(
        pool.clone(),
        sender.clone(),
        &HashMap::from([(world_reader.address, ContractType::WORLD)]),
    )
    .await
    .unwrap();

    let _ = bootstrap_engine(world_reader, db.clone(), provider).await.unwrap();

    let _block_timestamp = 1710754478_u64;
    let models = sqlx::query("SELECT * FROM models").fetch_all(&pool).await.unwrap();
    assert_eq!(models.len(), 8);

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
    let (mut executor, sender) = Executor::new(pool.clone(), shutdown_tx.clone()).await.unwrap();
    tokio::spawn(async move {
        executor.run().await.unwrap();
    });

    let db = Sql::new(
        pool.clone(),
        sender.clone(),
        &HashMap::from([(world_reader.address, ContractType::WORLD)]),
    )
    .await
    .unwrap();

    let _ = bootstrap_engine(world_reader, db.clone(), provider).await;

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
    let (mut executor, sender) = Executor::new(pool.clone(), shutdown_tx.clone()).await.unwrap();
    tokio::spawn(async move {
        executor.run().await.unwrap();
    });

    let db = Sql::new(
        pool.clone(),
        sender.clone(),
        &HashMap::from([(world_reader.address, ContractType::WORLD)]),
    )
    .await
    .unwrap();

    let _ = bootstrap_engine(world_reader, db.clone(), Arc::clone(&provider)).await.unwrap();
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
