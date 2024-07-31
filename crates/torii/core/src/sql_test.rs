use std::str::FromStr;

use dojo_test_utils::compiler::CompilerTestSetup;
use dojo_world::contracts::naming::compute_selector_from_names;
use dojo_world::contracts::world::WorldContractReader;
use dojo_world::migration::TxnConfig;
use dojo_world::utils::{TransactionExt, TransactionWaiter};
use katana_runner::KatanaRunner;
use scarb::compiler::Profile;
use sozo_ops::migration;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use starknet::accounts::{Account, Call, ConnectedAccount};
use starknet::core::types::{BlockId, BlockTag, Felt};
use starknet::core::utils::get_selector_from_name;
use starknet::providers::Provider;
use starknet_crypto::poseidon_hash_many;
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
    P: Provider + Send + Sync + core::fmt::Debug,
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

    let setup = CompilerTestSetup::from_examples("../../dojo-core", "../../../examples/");
    let config = setup.build_test_config("spawn-and-move", Profile::DEV);

    let ws = scarb::ops::read_workspace(config.manifest_path(), &config).unwrap();

    let sequencer = KatanaRunner::new().expect("Failed to start runner.");
    let account = sequencer.account(0);

    let migration_output = migration::migrate(
        &ws,
        None,
        sequencer.url().to_string(),
        account,
        "dojo_examples",
        false,
        TxnConfig::init_wait(),
        None,
    )
    .await
    .unwrap()
    .unwrap();

    let account = sequencer.account(0);
    // spawn
    let tx = &account
        .execute_v1(vec![Call {
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

    TransactionWaiter::new(tx.transaction_hash, &account.provider()).await.unwrap();

    let world_reader = WorldContractReader::new(migration_output.world_address, account.provider());

    let mut db = Sql::new(
        pool.clone(),
        world_reader.address,
        account
            .provider()
            .get_class_hash_at(BlockId::Tag(BlockTag::Pending), world_reader.address)
            .await
            .unwrap(),
    )
    .await
    .unwrap();

    let _ = bootstrap_engine(world_reader, db.clone(), account.provider()).await;

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

    let sequencer = KatanaRunner::new().expect("Failed to start runner.");
    let account = sequencer.account(0);

    let migration_output = migration::migrate(
        &ws,
        None,
        sequencer.url().to_string(),
        account,
        "dojo_examples",
        false,
        TxnConfig::init_wait(),
        None,
    )
    .await
    .unwrap()
    .unwrap();

    let account = sequencer.account(0);
    // spawn
    account
        .execute_v1(vec![Call {
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
        .execute_v1(vec![Call {
            to: migration_output
                .contracts
                .first()
                .expect("shouldn't be empty")
                .as_ref()
                .expect("should be deployed")
                .contract_address,
            selector: get_selector_from_name("set_player_config").unwrap(),
            // Empty ByteArray.
            calldata: vec![Felt::ZERO, Felt::ZERO, Felt::ZERO],
        }])
        .send_with_cfg(&TxnConfig::init_wait())
        .await
        .unwrap();

    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    account
        .execute_v1(vec![Call {
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

    let world_reader = WorldContractReader::new(migration_output.world_address, account.provider());

    let mut db = Sql::new(
        pool.clone(),
        world_reader.address,
        account
            .provider()
            .get_class_hash_at(BlockId::Tag(BlockTag::Pending), world_reader.address)
            .await
            .unwrap(),
    )
    .await
    .unwrap();

    let _ = bootstrap_engine(world_reader, db.clone(), account.provider()).await;

    assert_eq!(count_table("dojo_examples-PlayerConfig", &pool).await, 0);
    assert_eq!(count_table("dojo_examples-PlayerConfig$favorite_item", &pool).await, 0);
    assert_eq!(count_table("dojo_examples-PlayerConfig$items", &pool).await, 0);

    // TODO: check how we can have a test that is more chronological with Torii re-syncing
    // to ensure we can test intermediate states.

    db.execute().await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_entity_keys() {
    let options =
        SqliteConnectOptions::from_str("sqlite::memory:").unwrap().create_if_missing(true);
    let pool = SqlitePoolOptions::new().max_connections(5).connect_with(options).await.unwrap();
    sqlx::migrate!("../migrations").run(&pool).await.unwrap();

    let setup = CompilerTestSetup::from_examples("../../dojo-core", "../../../examples/");
    let config = setup.build_test_config("spawn-and-move", Profile::DEV);

    let ws = scarb::ops::read_workspace(config.manifest_path(), &config).unwrap();

    let sequencer = KatanaRunner::new().expect("Failed to start runner.");
    let account = sequencer.account(0);

    let migration_output = migration::migrate(
        &ws,
        None,
        sequencer.url().to_string(),
        account,
        "dojo_examples",
        false,
        TxnConfig::init_wait(),
        None,
    )
    .await
    .unwrap()
    .unwrap();

    let account = sequencer.account(0);
    // spawn
    account
        .execute_v1(vec![Call {
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

    let world_reader = WorldContractReader::new(migration_output.world_address, account.provider());

    let mut db = Sql::new(
        pool.clone(),
        world_reader.address,
        account
            .provider()
            .get_class_hash_at(BlockId::Tag(BlockTag::Pending), world_reader.address)
            .await
            .unwrap(),
    )
    .await
    .unwrap();

    let _ = bootstrap_engine(world_reader, db.clone(), account.provider()).await;

    let keys = db.get_entity_keys_def("dojo_examples-Moves").await.unwrap();
    assert_eq!(keys, vec![("player".to_string(), "ContractAddress".to_string()),]);

    let entity_id = poseidon_hash_many(&[account.address()]);

    let keys = db.get_entity_keys(entity_id, "dojo_examples-Moves").await.unwrap();
    assert_eq!(keys, vec![account.address()]);

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
