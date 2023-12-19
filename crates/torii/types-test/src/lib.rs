use std::str::FromStr;

use anyhow::Result;
use dojo_test_utils::compiler::build_test_config;
use dojo_test_utils::migration::prepare_migration;
use dojo_test_utils::sequencer::{
    get_default_test_starknet_config, SequencerConfig, TestSequencer,
};
use dojo_world::contracts::WorldContractReader;
use dojo_world::manifest::Manifest;
use dojo_world::utils::TransactionWaiter;
use scarb::ops;
use sozo::ops::migration::execute_strategy;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use starknet::accounts::{Account, Call};
use starknet::core::types::{BlockId, BlockTag, FieldElement, InvokeTransactionResult};
use starknet::macros::selector;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::JsonRpcClient;
use tokio::sync::broadcast;
use torii_core::engine::{Engine, EngineConfig, Processors};
use torii_core::processors::register_model::RegisterModelProcessor;
use torii_core::processors::store_set_record::StoreSetRecordProcessor;
use torii_core::sql::Sql;

pub async fn spinup_types_test() -> Result<SqlitePool> {
    // change sqlite::memory: to sqlite:~/.test.db to dump database to disk
    let options = SqliteConnectOptions::from_str("sqlite::memory:")?.create_if_missing(true);
    let pool = SqlitePoolOptions::new().max_connections(5).connect_with(options).await.unwrap();
    sqlx::migrate!("../migrations").run(&pool).await.unwrap();

    let migration = prepare_migration("../types-test/target/dev".into()).unwrap();
    let config = build_test_config("../types-test/Scarb.toml").unwrap();
    let mut db = Sql::new(pool.clone(), migration.world_address().unwrap()).await.unwrap();

    let sequencer =
        TestSequencer::start(SequencerConfig::default(), get_default_test_starknet_config()).await;

    let mut account = sequencer.account();
    account.set_block_id(BlockId::Tag(BlockTag::Pending));

    let provider = JsonRpcClient::new(HttpTransport::new(sequencer.url()));
    let world = WorldContractReader::new(migration.world_address().unwrap(), &provider);
    let ws = ops::read_workspace(config.manifest_path(), &config)
        .unwrap_or_else(|op| panic!("Error building workspace: {op:?}"));

    execute_strategy(&ws, &migration, &account, None).await.unwrap();

    let manifest =
        Manifest::load_from_remote(&provider, migration.world_address().unwrap()).await.unwrap();

    //  Execute `create` and insert 10 records into storage
    let records_contract =
        manifest.contracts.iter().find(|contract| contract.name.eq("records")).unwrap();
    let InvokeTransactionResult { transaction_hash } = account
        .execute(vec![Call {
            calldata: vec![FieldElement::from_str("0xa").unwrap()],
            to: records_contract.address.unwrap(),
            selector: selector!("create"),
        }])
        .send()
        .await
        .unwrap();

    TransactionWaiter::new(transaction_hash, &provider).await?;

    let (shutdown_tx, _) = broadcast::channel(1);
    let mut engine = Engine::new(
        world,
        &mut db,
        &provider,
        Processors {
            event: vec![Box::new(RegisterModelProcessor), Box::new(StoreSetRecordProcessor)],
            ..Processors::default()
        },
        EngineConfig::default(),
        shutdown_tx,
        None,
    );

    let _ = engine.sync_to_head(0).await?;

    Ok(pool)
}
