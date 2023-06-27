use serde_json::Value;
use sqlx::SqlitePool;
use camino::Utf8PathBuf;
use starknet::core::types::FieldElement;

use crate::state::sql::{Executable, Sql};
use crate::state::State;

use crate::graphql::schema::build_schema;

#[allow(dead_code)]
pub async fn run_graphql_query(pool: &SqlitePool, query: &str) -> Value {
    let schema = build_schema(pool).await.unwrap();
    let res = schema.execute(query).await;

    assert!(res.errors.is_empty(), "GraphQL query returned errors: {:?}", res.errors);
    serde_json::to_value(res.data).expect("Failed to serialize GraphQL response")
}

pub async fn entity_fixtures(pool: &SqlitePool) {
    let manifest = dojo_world::manifest::Manifest::load_from_path(
        Utf8PathBuf::from_path_buf("../../examples/ecs/target/dev/manifest.json".into())
            .unwrap(),
    )
    .unwrap();

    let state = Sql::new(pool.clone(), FieldElement::ZERO).await.unwrap();
    state.load_from_manifest(manifest).await.unwrap();

    // Set moves entity
    let key = vec![FieldElement::ONE];
    let partition = FieldElement::from_hex_be("0xdead").unwrap();
    let values = vec![FieldElement::from_hex_be("0xa").unwrap()];
    state.set_entity("moves".to_string(), partition, key, values).await.unwrap();

    // Set position entity
    let key = vec![FieldElement::TWO];
    let partition = FieldElement::from_hex_be("0xbeef").unwrap();
    let values = vec![
        FieldElement::from_hex_be("0x2a").unwrap(),
        FieldElement::from_hex_be("0x45").unwrap(),
    ];
    state.set_entity("position".to_string(), partition, key, values).await.unwrap();
    state.execute().await.unwrap();
}