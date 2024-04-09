
use std::{str::FromStr, sync::Arc};

use dojo_test_utils::{
    compiler::build_test_config,
    migration::prepare_migration,
    sequencer::{get_default_test_starknet_config, SequencerConfig, TestSequencer},
};
use dojo_world::{contracts::WorldContractReader, utils::TransactionWaiter};
use scarb::ops;
use sozo_ops::migration::execute_strategy;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use starknet::{
    accounts::{Account, Call},
    core::{
        types::{BlockId, BlockTag},
        utils::get_selector_from_name,
    },
    providers::{jsonrpc::HttpTransport, JsonRpcClient},
};
use tokio::sync::broadcast;
use torii_core::{
    engine::{Engine, EngineConfig, Processors},
    processors::{
        register_model::RegisterModelProcessor, store_set_record::StoreSetRecordProcessor,
    },
    sql::Sql,
};

use crate::server::DojoWorld;

#[tokio::test(flavor = "multi_thread")]
async fn test_entities_queries() {
    let options =
        SqliteConnectOptions::from_str("sqlite::memory:").unwrap().create_if_missing(true);
    let pool = SqlitePoolOptions::new().max_connections(5).connect_with(options).await.unwrap();
    sqlx::migrate!("../migrations").run(&pool).await.unwrap();
    let base_path = "../../../examples/spawn-and-move";
    let target_path = format!("{}/target/dev", base_path);
    let mut migration = prepare_migration(base_path.into(), target_path.into()).unwrap();
    let sequencer =
        TestSequencer::start(SequencerConfig::default(), get_default_test_starknet_config()).await;
    let provider = Arc::new(JsonRpcClient::new(HttpTransport::new(sequencer.url())));
    let world = WorldContractReader::new(migration.world_address().unwrap(), &provider);

    let mut account = sequencer.account();
    account.set_block_id(BlockId::Tag(BlockTag::Pending));

    let config = build_test_config("../../../examples/spawn-and-move/Scarb.toml").unwrap();
    let ws = ops::read_workspace(config.manifest_path(), &config)
        .unwrap_or_else(|op| panic!("Error building workspace: {op:?}"));
    execute_strategy(&ws, &mut migration, &account, None).await.unwrap();

    // spawn
    let tx = account
        .execute(vec![Call {
            to: migration.contracts.first().unwrap().contract_address,
            selector: get_selector_from_name("spawn").unwrap(),
            calldata: vec![],
        }])
        .send()
        .await
        .unwrap();

    TransactionWaiter::new(tx.transaction_hash, &provider).await.unwrap();

    let db = Sql::new(pool.clone(), migration.world_address().unwrap()).await.unwrap();

    let (shutdown_tx, _) = broadcast::channel(1);
    let mut engine = Engine::new(
        world,
        db.clone(),
        &provider,
        Processors {
            event: vec![Box::new(RegisterModelProcessor), Box::new(StoreSetRecordProcessor)],
            ..Processors::default()
        },
        EngineConfig::default(),
        shutdown_tx,
        None,
    );

    let _ = engine.sync_to_head(0).await.unwrap();

    let (_, receiver) = tokio::sync::mpsc::channel(1);
    let grpc =
        DojoWorld::new(db.pool, receiver, migration.world_address.unwrap(), provider.clone());

    // let entities = grpc.query_by_keys("entities", "entity_model", KeysClause {

    // })
}
