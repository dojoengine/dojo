use camino::Utf8PathBuf;
use serde::Deserialize;
use serde_json::Value;
use sqlx::SqlitePool;
use starknet::core::types::FieldElement;
use tokio_stream::StreamExt;
use torii_core::sql::{Executable, Sql};

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

pub async fn run_graphql_subscription(
    pool: &SqlitePool,
    subscription: &str,
) -> async_graphql::Value {
    // Build dynamic schema
    let schema = build_schema(pool).await.unwrap();
    schema.execute_stream(subscription).next().await.unwrap().into_result().unwrap().data
    // fn subscribe() is called from inside dynamic subscription
}

pub async fn entity_fixtures(pool: &SqlitePool) {
    let state = init(pool).await;

    // Set entity with one moves model
    // remaining: 10, last_direction: 0
    let key = vec![FieldElement::ONE];
    let moves_values = vec![FieldElement::from_hex_be("0xa").unwrap(), FieldElement::ZERO];
    state.set_entity("Moves".to_string(), key, moves_values.clone()).await.unwrap();

    // Set entity with one position model
    // x: 42
    // y: 69
    let key = vec![FieldElement::TWO];
    let position_values = vec![
        FieldElement::from_hex_be("0x2a").unwrap(),
        FieldElement::from_hex_be("0x45").unwrap(),
    ];
    state.set_entity("Position".to_string(), key, position_values.clone()).await.unwrap();

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
    state.set_entity("Moves".to_string(), key.clone(), moves_values).await.unwrap();
    state.set_entity("Position".to_string(), key, position_values).await.unwrap();

    state.execute().await.unwrap();
}

pub async fn init(pool: &SqlitePool) -> Sql {
    let manifest = dojo_world::manifest::Manifest::load_from_path(
        Utf8PathBuf::from_path_buf("../../../examples/ecs/target/dev/manifest.json".into())
            .unwrap(),
    )
    .unwrap();

    let state = Sql::new(pool.clone(), FieldElement::ZERO).await.unwrap();
    state.load_from_manifest(manifest).await.unwrap();
    state
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
