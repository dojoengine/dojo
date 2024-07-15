use std::str::FromStr;
use std::sync::Arc;

use dojo_test_utils::compiler::CompilerTestSetup;
use dojo_test_utils::migration::prepare_migration;
use dojo_world::contracts::WorldContractReader;
use dojo_world::metadata::{dojo_metadata_from_workspace, get_default_namespace_from_ws};
use dojo_world::migration::TxnConfig;
use dojo_world::utils::TransactionWaiter;
use katana_runner::KatanaRunner;
use scarb::compiler::Profile;
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
use torii_core::engine::{Engine, EngineConfig, Processors};
use torii_core::processors::register_model::RegisterModelProcessor;
use torii_core::processors::store_set_record::StoreSetRecordProcessor;
use torii_core::sql::Sql;

use crate::proto::types::KeysClause;
use crate::server::DojoWorld;
use crate::types::schema::Entity;

#[tokio::test(flavor = "multi_thread")]
async fn test_entities_queries() {
    let options = SqliteConnectOptions::from_str("sqlite::memory:")
        .unwrap()
        .create_if_missing(true)
        .with_regexp();
    let pool = SqlitePoolOptions::new().max_connections(5).connect_with(options).await.unwrap();
    sqlx::migrate!("../migrations").run(&pool).await.unwrap();

    let setup = CompilerTestSetup::from_examples("../../dojo-core", "../../../examples/");
    let config = setup.build_test_config("spawn-and-move", Profile::DEV);

    let ws = ops::read_workspace(config.manifest_path(), &config)
        .unwrap_or_else(|op| panic!("Error building workspace: {op:?}"));
    let dojo_metadata =
        dojo_metadata_from_workspace(&ws).expect("No current package with dojo metadata found.");

    let target_path = ws.target_dir().path_existent().unwrap().join(config.profile().to_string());

    let default_namespace = get_default_namespace_from_ws(&ws).unwrap();

    let mut migration = prepare_migration(
        config.manifest_path().parent().unwrap().into(),
        target_path,
        dojo_metadata.skip_migration,
        &default_namespace,
    )
    .unwrap();
    migration.resolve_variable(migration.world_address().unwrap()).unwrap();

    let sequencer = KatanaRunner::new().expect("Fail to start runner");

    let provider = Arc::new(JsonRpcClient::new(HttpTransport::new(sequencer.url())));

    let world = WorldContractReader::new(migration.world_address().unwrap(), &provider);

    let account = sequencer.account(0);

    let migration_output =
        execute_strategy(&ws, &migration, &account, TxnConfig::init_wait()).await.unwrap();

    let world_address = migration_output.world_address;

    println!("output {:?}", migration_output);

    // spawn
    let tx = account
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

    TransactionWaiter::new(tx.transaction_hash, &provider).await.unwrap();

    let db = Sql::new(
        pool.clone(),
        world_address,
        provider.get_class_hash_at(BlockId::Tag(BlockTag::Pending), world_address).await.unwrap(),
    )
    .await
    .unwrap();

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

    let _ = engine.sync_to_head(0, None).await.unwrap();

    let (_, receiver) = tokio::sync::mpsc::channel(1);
    let grpc = DojoWorld::new(db.pool, receiver, world_address, provider.clone());

    let entities = grpc
        .query_by_keys(
            "entities",
            "entity_model",
            "entity_id",
            &KeysClause {
                keys: vec![account.address().to_bytes_be().to_vec()],
                pattern_matching: 0,
                models: vec![],
            },
            Some(1),
            None,
        )
        .await
        .unwrap()
        .0;

    assert_eq!(entities.len(), 1);

    let entity: Entity = entities.first().unwrap().clone().try_into().unwrap();
    assert_eq!(entity.models.first().unwrap().name, "dojo_examples-Position");
    assert_eq!(entity.models.get(1).unwrap().name, "dojo_examples-Moves");
    assert_eq!(entity.hashed_keys, poseidon_hash_many(&[account.address()]));
}
