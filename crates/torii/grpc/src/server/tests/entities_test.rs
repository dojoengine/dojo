use std::str::FromStr;
use std::sync::Arc;

use camino::Utf8PathBuf;
use dojo_test_utils::compiler::CompilerTestSetup;
use dojo_test_utils::migration::{copy_spawn_and_move_db, prepare_migration_with_world_and_seed};
use dojo_world::contracts::WorldContractReader;
use dojo_world::utils::TransactionWaiter;
use katana_runner::{KatanaRunner, KatanaRunnerConfig};
use scarb::compiler::Profile;
use scarb::ops;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use starknet::accounts::{Account, Call};
use starknet::core::types::{BlockId, BlockTag};
use starknet::core::utils::{get_contract_address, get_selector_from_name};
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

    let manifest_path = Utf8PathBuf::from(config.manifest_path().parent().unwrap());
    let target_path = ws.target_dir().path_existent().unwrap().join(config.profile().to_string());

    let seq_config = KatanaRunnerConfig::default().with_db_dir(copy_spawn_and_move_db().as_str());
    let sequencer = KatanaRunner::new_with_config(seq_config).expect("Failed to start runner.");
    let account = sequencer.account(0);

    let (strat, _) = prepare_migration_with_world_and_seed(
        manifest_path,
        target_path,
        None,
        "dojo_examples",
        "dojo_examples",
    )
    .unwrap();

    let provider = Arc::new(JsonRpcClient::new(HttpTransport::new(sequencer.url())));

    let world = WorldContractReader::new(strat.world_address, &provider);

    let actions = strat.contracts.first().unwrap();
    let actions_address = get_contract_address(
        actions.salt,
        strat.base.as_ref().unwrap().diff.local_class_hash,
        &[],
        strat.world_address,
    );

    // spawn
    let tx = account
        .execute_v1(vec![Call {
            to: actions_address,
            selector: get_selector_from_name("spawn").unwrap(),
            calldata: vec![],
        }])
        .send()
        .await
        .unwrap();

    TransactionWaiter::new(tx.transaction_hash, &provider).await.unwrap();

    let db = Sql::new(
        pool.clone(),
        strat.world_address,
        provider
            .get_class_hash_at(BlockId::Tag(BlockTag::Pending), strat.world_address)
            .await
            .unwrap(),
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
    let grpc = DojoWorld::new(db.pool, receiver, strat.world_address, provider.clone());

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
