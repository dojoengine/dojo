use std::str::FromStr;
use std::sync::Arc;

use cainome::cairo_serde::ContractAddress;
use camino::Utf8PathBuf;
use dojo_test_utils::compiler::CompilerTestSetup;
use dojo_test_utils::migration::{copy_spawn_and_move_db, prepare_migration_with_world_and_seed};
use dojo_utils::{TransactionExt, TransactionWaiter, TxnConfig};
use dojo_world::contracts::naming::{compute_bytearray_hash, compute_selector_from_names};
use dojo_world::contracts::world::{WorldContract, WorldContractReader};
use katana_runner::{KatanaRunner, KatanaRunnerConfig};
use scarb::compiler::Profile;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use starknet::accounts::Account;
use starknet::core::types::{Call, Felt};
use starknet::core::utils::{get_contract_address, get_selector_from_name};
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::{JsonRpcClient, Provider};
use starknet_crypto::poseidon_hash_many;
use tokio::sync::broadcast;

use crate::engine::{Engine, EngineConfig, Processors};
use crate::processors::generate_event_processors_map;
use crate::processors::register_model::RegisterModelProcessor;
use crate::processors::store_del_record::StoreDelRecordProcessor;
use crate::processors::store_set_record::StoreSetRecordProcessor;
use crate::processors::store_update_member::StoreUpdateMemberProcessor;
use crate::processors::store_update_record::StoreUpdateRecordProcessor;
use crate::sql::Sql;

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
    let mut engine = Engine::new(
        world,
        db,
        provider,
        Processors {
            event: generate_event_processors_map(vec![
                Arc::new(RegisterModelProcessor),
                Arc::new(StoreSetRecordProcessor),
                Arc::new(StoreUpdateRecordProcessor),
                Arc::new(StoreUpdateMemberProcessor),
                Arc::new(StoreDelRecordProcessor),
            ])?,
            ..Processors::default()
        },
        EngineConfig::default(),
        shutdown_tx,
        None,
    );

    let data = engine.fetch_range(0, to, None).await.unwrap();
    engine.process_range(data).await.unwrap();

    Ok(engine)
}

#[tokio::test(flavor = "multi_thread")]
async fn test_load_from_remote() {
    let options =
        SqliteConnectOptions::from_str("sqlite::memory:").unwrap().create_if_missing(true);
    let pool = SqlitePoolOptions::new().max_connections(5).connect_with(options).await.unwrap();
    sqlx::migrate!("../migrations").run(&pool).await.unwrap();

    let setup = CompilerTestSetup::from_examples("../../dojo-core", "../../../examples/");
    let config = setup.build_test_config("spawn-and-move", Profile::DEV);

    let ws = scarb::ops::read_workspace(config.manifest_path(), &config).unwrap();
    let manifest_path = Utf8PathBuf::from(config.manifest_path().parent().unwrap());
    let target_dir = Utf8PathBuf::from(ws.target_dir().to_string()).join("dev");

    let seq_config = KatanaRunnerConfig { n_accounts: 10, ..Default::default() }
        .with_db_dir(copy_spawn_and_move_db().as_str());

    let sequencer = KatanaRunner::new_with_config(seq_config).expect("Failed to start runner.");
    let account = sequencer.account(0);
    let provider = Arc::new(JsonRpcClient::new(HttpTransport::new(sequencer.url())));

    let (strat, _) = prepare_migration_with_world_and_seed(
        manifest_path,
        target_dir,
        None,
        "dojo_examples",
        "dojo_examples",
    )
    .unwrap();

    let actions = strat.contracts.first().unwrap();
    let actions_address = get_contract_address(
        actions.salt,
        strat.base.as_ref().unwrap().diff.local_class_hash,
        &[],
        strat.world_address,
    );

    let world = WorldContract::new(strat.world_address, &account);

    let res = world
        .grant_writer(&compute_bytearray_hash("dojo_examples"), &ContractAddress(actions_address))
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

    let world_reader = WorldContractReader::new(strat.world_address, Arc::clone(&provider));

    let mut db = Sql::new(pool.clone(), world_reader.address).await.unwrap();

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

    assert_eq!(id, format!("{:#x}", compute_selector_from_names("dojo_examples", "Position")));
    assert_eq!(name, "Position");
    assert_eq!(namespace, "dojo_examples");
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

    assert_eq!(id, format!("{:#x}", compute_selector_from_names("dojo_examples", "Moves")));
    assert_eq!(name, "Moves");
    assert_eq!(namespace, "dojo_examples");
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

    assert_eq!(id, format!("{:#x}", compute_selector_from_names("dojo_examples", "PlayerConfig")));
    assert_eq!(name, "PlayerConfig");
    assert_eq!(namespace, "dojo_examples");
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

    let setup = CompilerTestSetup::from_examples("../../dojo-core", "../../../examples/");
    let config = setup.build_test_config("spawn-and-move", Profile::DEV);

    let ws = scarb::ops::read_workspace(config.manifest_path(), &config).unwrap();
    let manifest_path = Utf8PathBuf::from(config.manifest_path().parent().unwrap());
    let target_dir = Utf8PathBuf::from(ws.target_dir().to_string()).join("dev");

    let seq_config = KatanaRunnerConfig { n_accounts: 10, ..Default::default() }
        .with_db_dir(copy_spawn_and_move_db().as_str());

    let sequencer = KatanaRunner::new_with_config(seq_config).expect("Failed to start runner.");
    let account = sequencer.account(0);
    let provider = Arc::new(JsonRpcClient::new(HttpTransport::new(sequencer.url())));

    let (strat, _) = prepare_migration_with_world_and_seed(
        manifest_path,
        target_dir,
        None,
        "dojo_examples",
        "dojo_examples",
    )
    .unwrap();
    let actions = strat.contracts.first().unwrap();
    let actions_address = get_contract_address(
        actions.salt,
        strat.base.as_ref().unwrap().diff.local_class_hash,
        &[],
        strat.world_address,
    );

    let world = WorldContract::new(strat.world_address, &account);

    let res = world
        .grant_writer(&compute_bytearray_hash("dojo_examples"), &ContractAddress(actions_address))
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

    let world_reader = WorldContractReader::new(strat.world_address, Arc::clone(&provider));

    let mut db = Sql::new(pool.clone(), world_reader.address).await.unwrap();

    let _ = bootstrap_engine(world_reader, db.clone(), provider).await;

    assert_eq!(count_table("dojo_examples-PlayerConfig", &pool).await, 0);
    assert_eq!(count_table("dojo_examples-PlayerConfig$favorite_item", &pool).await, 0);
    assert_eq!(count_table("dojo_examples-PlayerConfig$items", &pool).await, 0);

    // TODO: check how we can have a test that is more chronological with Torii re-syncing
    // to ensure we can test intermediate states.

    db.execute().await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn test_update_with_set_record() {
    let options =
        SqliteConnectOptions::from_str("sqlite::memory:").unwrap().create_if_missing(true);
    let pool = SqlitePoolOptions::new().max_connections(5).connect_with(options).await.unwrap();
    sqlx::migrate!("../migrations").run(&pool).await.unwrap();

    let setup = CompilerTestSetup::from_examples("../../dojo-core", "../../../examples/");
    let config = setup.build_test_config("spawn-and-move", Profile::DEV);

    let ws = scarb::ops::read_workspace(config.manifest_path(), &config).unwrap();
    let manifest_path = Utf8PathBuf::from(config.manifest_path().parent().unwrap());
    let target_dir = Utf8PathBuf::from(ws.target_dir().to_string()).join("dev");

    let seq_config = KatanaRunnerConfig { n_accounts: 10, ..Default::default() }
        .with_db_dir(copy_spawn_and_move_db().as_str());
    let sequencer = KatanaRunner::new_with_config(seq_config).expect("Failed to start runner.");

    let (strat, _) = prepare_migration_with_world_and_seed(
        manifest_path,
        target_dir,
        None,
        "dojo_examples",
        "dojo_examples",
    )
    .unwrap();

    let actions = strat.contracts.first().unwrap();
    let actions_address = get_contract_address(
        actions.salt,
        strat.base.as_ref().unwrap().diff.local_class_hash,
        &[],
        strat.world_address,
    );

    let account = sequencer.account(0);
    let provider = Arc::new(JsonRpcClient::new(HttpTransport::new(sequencer.url())));

    let world = WorldContract::new(strat.world_address, &account);

    let res = world
        .grant_writer(&compute_bytearray_hash("dojo_examples"), &ContractAddress(actions_address))
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

    let world_reader = WorldContractReader::new(strat.world_address, Arc::clone(&provider));

    let mut db = Sql::new(pool.clone(), world_reader.address).await.unwrap();

    let _ = bootstrap_engine(world_reader, db.clone(), Arc::clone(&provider)).await.unwrap();

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
    let count_query = format!("SELECT COUNT(*) FROM [{}]", table_name);
    let count: (i64,) = sqlx::query_as(&count_query).fetch_one(pool).await.unwrap();

    count.0
}
