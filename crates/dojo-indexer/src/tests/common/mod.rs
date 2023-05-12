use std::sync::Arc;

use juniper::{EmptyMutation, EmptySubscription, Variables};
use serde_json::Value;
use sqlx::SqlitePool;

use crate::graphql::server::{Context, Schema};
use crate::graphql::Query;

#[allow(dead_code)]
pub async fn run_graphql_query(pool: &SqlitePool, query: &str) -> Value {
    let context = Context {
        schema: Arc::new(Schema::new(
            Query,
            EmptyMutation::<Context>::new(),
            EmptySubscription::<Context>::new(),
        )),
        pool: Arc::new(pool.clone()),
    };

    let schema = context.schema.clone();
    let (result, error) = juniper::execute(query, None, &schema, &Variables::new(), &context)
        .await
        .unwrap_or_else(|error| panic!("GraphQL query failed: {}", error));

    assert!(error.is_empty(), "GraphQL query returned errors: {:?}", error);

    serde_json::from_str(&result.to_string()).expect("Failed to parse GraphQL query result as JSON")
}
