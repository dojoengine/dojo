use dojo_test_utils::migration::prepare_migration;
use dojo_test_utils::sequencer::{
    get_default_test_starknet_config, SequencerConfig, TestSequencer,
};
use dojo_world::migration::strategy::MigrationStrategy;
use scarb_ui::{OutputFormat, Ui, Verbosity};
use sozo::ops::migration::execute_strategy;
use sqlx::sqlite::SqlitePoolOptions;
use starknet::core::types::{BlockId, BlockTag, Event, FieldElement};
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::JsonRpcClient;
use torii_client::contract::world::WorldContractReader;

use crate::engine::{Engine, EngineConfig, Processors};
use crate::processors::register_model::RegisterModelProcessor;
use crate::processors::register_system::RegisterSystemProcessor;
use crate::processors::store_set_record::StoreSetRecordProcessor;
use crate::sql::Sql;

pub async fn bootstrap_engine<'a>(
    world: &'a WorldContractReader<'a, JsonRpcClient<HttpTransport>>,
    db: &'a mut Sql,
    provider: &'a JsonRpcClient<HttpTransport>,
    migration: &MigrationStrategy,
    sequencer: &TestSequencer,
) -> Result<Engine<'a, JsonRpcClient<HttpTransport>>, Box<dyn std::error::Error>> {
    let mut account = sequencer.account();
    account.set_block_id(BlockId::Tag(BlockTag::Pending));

    let ui = Ui::new(Verbosity::Verbose, OutputFormat::Text);
    execute_strategy(migration, &account, &ui, None).await.unwrap();

    let mut engine = Engine::new(
        world,
        db,
        provider,
        Processors {
            event: vec![
                Box::new(RegisterModelProcessor),
                Box::new(RegisterSystemProcessor),
                Box::new(StoreSetRecordProcessor),
            ],
            ..Processors::default()
        },
        EngineConfig::default(),
        None,
    );

    let _ = engine.sync_to_head(0).await?;

    Ok(engine)
}

#[tokio::test(flavor = "multi_thread")]
async fn test_load_from_remote() {
    let pool =
        SqlitePoolOptions::new().max_connections(5).connect("sqlite::memory:").await.unwrap();
    sqlx::migrate!("../migrations").run(&pool).await.unwrap();
    let migration = prepare_migration("../../../examples/ecs/target/dev".into()).unwrap();
    let sequencer =
        TestSequencer::start(SequencerConfig::default(), get_default_test_starknet_config()).await;
    let provider = JsonRpcClient::new(HttpTransport::new(sequencer.url()));
    let world = WorldContractReader::new(migration.world_address().unwrap(), &provider);

    let mut db = Sql::new(pool.clone(), migration.world_address().unwrap()).await.unwrap();
    let _ = bootstrap_engine(&world, &mut db, &provider, &migration, &sequencer).await;

    let models = sqlx::query("SELECT * FROM models").fetch_all(&pool).await.unwrap();
    assert_eq!(models.len(), 2);

    let (id, name): (String, String) =
        sqlx::query_as("SELECT id, name FROM models WHERE id = 'Position'")
            .fetch_one(&pool)
            .await
            .unwrap();

    assert_eq!(id, "Position");
    assert_eq!(name, "Position");

    let (id, name): (String, String) =
        sqlx::query_as("SELECT id, name FROM models WHERE id = 'Moves'")
            .fetch_one(&pool)
            .await
            .unwrap();

    assert_eq!(id, "Moves");
    assert_eq!(name, "Moves");

    db.store_event(
        &Event {
            from_address: FieldElement::ONE,
            keys: Vec::from([FieldElement::TWO]),
            data: Vec::from([FieldElement::TWO, FieldElement::THREE]),
        },
        0,
        FieldElement::THREE,
    );

    db.execute().await.unwrap();

    let keys = format!("{:#x}/", FieldElement::TWO);
    let query = format!("SELECT data, transaction_hash FROM events WHERE keys = '{}'", keys);
    let (data, tx_hash): (String, String) = sqlx::query_as(&query).fetch_one(&pool).await.unwrap();

    assert_eq!(data, format!("{:#x}/{:#x}/", FieldElement::TWO, FieldElement::THREE));
    assert_eq!(tx_hash, format!("{:#x}", FieldElement::THREE))
}
