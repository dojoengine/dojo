use camino::Utf8PathBuf;
use serde::Deserialize;
use serde_json::Value;
use sqlx::SqlitePool;
use starknet::core::types::FieldElement;

use crate::graphql::schema::build_schema;
use crate::state::sql::{Executable, Sql};
use crate::state::State;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entity {
    pub component_names: String,
    pub keys: Option<String>,
}

#[derive(Deserialize)]
pub struct Moves {
    pub __typename: String,
    pub remaining: u32,
}

#[derive(Deserialize)]
pub struct Position {
    pub __typename: String,
    pub x: u32,
    pub y: u32,
}

#[allow(dead_code)]
pub async fn run_graphql_query(pool: &SqlitePool, query: &str) -> Value {
    let schema = build_schema(pool).await.unwrap();
    let res = schema.execute(query).await;

    assert!(res.errors.is_empty(), "GraphQL query returned errors: {:?}", res.errors);
    serde_json::to_value(res.data).expect("Failed to serialize GraphQL response")
}

pub async fn entity_fixtures(pool: &SqlitePool) {
    let manifest = dojo_world::manifest::Manifest::load_from_path(
        Utf8PathBuf::from_path_buf("../../examples/ecs/target/dev/manifest.json".into()).unwrap(),
    )
    .unwrap();

    let state = Sql::new(pool.clone(), FieldElement::ZERO).await.unwrap();
    state.load_from_manifest(manifest).await.unwrap();

    // Set entity with one moves component
    let key = vec![FieldElement::ONE];
    let moves_values = vec![
        FieldElement::from_hex_be("0xbeef").unwrap(),
        FieldElement::from_hex_be("0xa").unwrap(),
    ];
    state.set_entity("Moves".to_string(), key, moves_values.clone()).await.unwrap();

    // Set entity with one position component
    let key = vec![FieldElement::TWO];
    let position_values = vec![
        FieldElement::from_hex_be("0xbeef").unwrap(),
        FieldElement::from_hex_be("0x2a").unwrap(),
        FieldElement::from_hex_be("0x45").unwrap(),
    ];
    state.set_entity("Position".to_string(), key, position_values.clone()).await.unwrap();

    // Set an entity with both moves and position components
    let key = vec![FieldElement::THREE];
    state.set_entity("Moves".to_string(), key.clone(), moves_values).await.unwrap();
    state.set_entity("Position".to_string(), key, position_values).await.unwrap();

    state.execute().await.unwrap();
}
