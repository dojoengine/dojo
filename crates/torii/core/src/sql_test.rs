use std::str::FromStr;

use dojo_test_utils::compiler::build_test_config;
use dojo_test_utils::migration::prepare_migration;
use dojo_test_utils::sequencer::{
    get_default_test_starknet_config, SequencerConfig, TestSequencer,
};
use dojo_world::contracts::world::WorldContractReader;
use dojo_world::migration::TxnConfig;
use dojo_world::utils::TransactionWaiter;
use scarb::ops;
use sozo_ops::migration::execute_strategy;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use starknet::accounts::{Account, Call};
use starknet::core::types::{BlockId, BlockTag};
use starknet::core::utils::get_selector_from_name;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::{JsonRpcClient, Provider};
use starknet_crypto::poseidon_hash_many;
use tokio::sync::broadcast;

use crate::engine::{Engine, EngineConfig, Processors};
use crate::processors::register_model::RegisterModelProcessor;
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
            event: vec![Box::new(RegisterModelProcessor), Box::new(StoreSetRecordProcessor)],
            ..Processors::default()
        },
        EngineConfig::default(),
        shutdown_tx,
        None,
    );

    let _ = engine.sync_to_head(0).await?;

    Ok(engine)
}

#[tokio::test(flavor = "multi_thread")]
async fn test_load_from_remote() {
    let options =
        SqliteConnectOptions::from_str("sqlite::memory:").unwrap().create_if_missing(true);
    let pool = SqlitePoolOptions::new().max_connections(5).connect_with(options).await.unwrap();
    sqlx::migrate!("../migrations").run(&pool).await.unwrap();
    let base_path = "../../../examples/spawn-and-move";
    let target_path = format!("{}/target/dev", base_path);
    let migration = prepare_migration(base_path.into(), target_path.into()).unwrap();
    let sequencer =
        TestSequencer::start(SequencerConfig::default(), get_default_test_starknet_config()).await;
    let provider = JsonRpcClient::new(HttpTransport::new(sequencer.url()));
    let world = WorldContractReader::new(migration.world_address().unwrap(), &provider);

    let mut account = sequencer.account();
    account.set_block_id(BlockId::Tag(BlockTag::Pending));

    let config = build_test_config("../../../examples/spawn-and-move/Scarb.toml").unwrap();
    let ws = ops::read_workspace(config.manifest_path(), &config)
        .unwrap_or_else(|op| panic!("Error building workspace: {op:?}"));
    let migration_output =
        execute_strategy(&ws, &migration, &account, TxnConfig::default()).await.unwrap();
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
    assert_eq!(models.len(), 4);

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
    assert_eq!(packed_size, 1);
    assert_eq!(unpacked_size, 2);

    // print all entities
    let entities = sqlx::query("SELECT * FROM entities").fetch_all(&pool).await.unwrap();
    assert_eq!(entities.len(), 1);

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
