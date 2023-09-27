use camino::Utf8PathBuf;
use dojo_test_utils::sequencer::TestSequencer;
use dojo_world::migration::strategy::MigrationStrategy;
use scarb_ui::{OutputFormat, Ui, Verbosity};
use serde::Deserialize;
use serde_json::Value;
use sozo::ops::migration::execute_strategy;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;
use starknet::core::types::{BlockId, BlockTag, FieldElement};
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::JsonRpcClient;
use tokio_stream::StreamExt;
use torii_client::contract::world::WorldContractReader;
use torii_core::engine::{Engine, EngineConfig, Processors};
use torii_core::processors::register_model::RegisterModelProcessor;
use torii_core::processors::register_system::RegisterSystemProcessor;
use torii_core::processors::store_set_record::StoreSetRecordProcessor;
use torii_core::sql::{Executable, Sql};

mod entities_test;
// mod models_test;
// mod subscription_test;

use crate::schema::build_schema;

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Connection<T> {
    pub total_count: i64,
    pub edges: Vec<Edge<T>>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Edge<T> {
    pub node: T,
    pub cursor: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Entity {
    pub model_names: String,
    pub keys: Option<Vec<String>>,
    pub created_at: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct Moves {
    pub __typename: String,
    pub remaining: u32,
    pub last_direction: u8,
    pub entity: Option<Entity>,
}

#[derive(Deserialize, Debug)]
pub struct Position {
    pub __typename: String,
    pub x: u32,
    pub y: u32,
    pub entity: Option<Entity>,
}

pub enum Paginate {
    Forward,
    Backward,
}

#[allow(dead_code)]
pub async fn run_graphql_query(pool: &SqlitePool, query: &str) -> Value {
    let schema = build_schema(pool).await.unwrap();
    let res = schema.execute(query).await;

    assert!(res.errors.is_empty(), "GraphQL query returned errors: {:?}", res.errors);
    serde_json::to_value(res.data).expect("Failed to serialize GraphQL response")
}

pub async fn create_pool() -> SqlitePool {
    let pool =
        SqlitePoolOptions::new().max_connections(5).connect("sqlite::memory:").await.unwrap();
    sqlx::migrate!("../migrations").run(&pool).await.unwrap();
    pool
}

pub async fn bootstrap_engine<'a>(
    world: &'a WorldContractReader<'a, JsonRpcClient<HttpTransport>>,
    db: &'a Sql,
    provider: &'a JsonRpcClient<HttpTransport>,
    migration: &MigrationStrategy,
    sequencer: &TestSequencer,
) -> Result<Engine<'a, JsonRpcClient<HttpTransport>>, Box<dyn std::error::Error>> {
    let mut account = sequencer.account();
    account.set_block_id(BlockId::Tag(BlockTag::Pending));

    let manifest = dojo_world::manifest::Manifest::load_from_path(
        Utf8PathBuf::from_path_buf("../../../examples/ecs/target/dev/manifest.json".into())
            .unwrap(),
    )
    .unwrap();

    db.load_from_manifest(manifest.clone()).await.unwrap();

    let ui = Ui::new(Verbosity::Verbose, OutputFormat::Text);
    execute_strategy(migration, &account, &ui, None).await.unwrap();

    let engine = Engine::new(
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
    );

    let _ = engine.sync_to_head(0).await?;

    Ok(engine)
}

#[allow(dead_code)]
pub async fn run_graphql_subscription(
    pool: &SqlitePool,
    subscription: &str,
) -> async_graphql::Value {
    // Build dynamic schema
    let schema = build_schema(pool).await.unwrap();
    schema.execute_stream(subscription).next().await.unwrap().into_result().unwrap().data
    // fn subscribe() is called from inside dynamic subscription
}

pub async fn entity_fixtures(db: &Sql) {
    // Set entity with one moves model
    // remaining: 10, last_direction: 0
    let key = vec![FieldElement::ONE];
    let moves_values = vec![FieldElement::from_hex_be("0xa").unwrap(), FieldElement::ZERO];
    db.set_entity("Moves".to_string(), key, moves_values.clone()).await.unwrap();

    // Set entity with one position model
    // x: 42
    // y: 69
    let key = vec![FieldElement::TWO];
    let position_values = vec![
        FieldElement::from_hex_be("0x2a").unwrap(),
        FieldElement::from_hex_be("0x45").unwrap(),
    ];
    db.set_entity("Position".to_string(), key, position_values.clone()).await.unwrap();

    // Set an entity with both moves and position models
    // remaining: 1, last_direction: 0
    // x: 69
    // y: 42
    let key = vec![FieldElement::THREE];
    let moves_values = vec![FieldElement::from_hex_be("0x1").unwrap(), FieldElement::ZERO];
    let position_values = vec![
        FieldElement::from_hex_be("0x45").unwrap(),
        FieldElement::from_hex_be("0x2a").unwrap(),
    ];
    db.set_entity("Moves".to_string(), key.clone(), moves_values).await.unwrap();
    db.set_entity("Position".to_string(), key, position_values).await.unwrap();

    db.execute().await.unwrap();
}

pub async fn paginate(
    pool: &SqlitePool,
    cursor: Option<String>,
    direction: Paginate,
    page_size: usize,
) -> Connection<Entity> {
    let (first_last, before_after) = match direction {
        Paginate::Forward => ("first", "after"),
        Paginate::Backward => ("last", "before"),
    };

    let cursor = cursor.map_or(String::new(), |c| format!(", {before_after}: \"{c}\""));
    let query = format!(
        "
        {{
            entities ({first_last}: {page_size} {cursor}) 
            {{
                totalCount
                edges {{
                    cursor
                    node {{
                        modelNames
                    }}
                }}
            }}
        }}
        "
    );

    let value = run_graphql_query(pool, &query).await;
    let entities = value.get("entities").ok_or("entities not found").unwrap();
    serde_json::from_value(entities.clone()).unwrap()
}
