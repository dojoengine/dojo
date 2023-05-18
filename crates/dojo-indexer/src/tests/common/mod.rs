use async_graphql::{EmptyMutation, EmptySubscription, Schema};
use serde_json::Value;
use sqlx::SqlitePool;

use crate::graphql::Query;

#[allow(dead_code)]
pub async fn run_graphql_query(pool: &SqlitePool, query: &str) -> Value {
    let schema = Schema::build(Query, EmptyMutation, EmptySubscription).data(pool.clone()).finish();

    let res = schema.execute(query).await;

    assert!(res.errors.is_empty(), "GraphQL query returned errors: {:?}", res.errors);

    serde_json::to_value(res.data).expect("Failed to serialize GraphQL response")
}
