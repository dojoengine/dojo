use std::convert::Infallible;

use async_graphql::dynamic::Schema;
use async_graphql::http::{playground_source, GraphQLPlaygroundConfig};
use async_graphql_warp::graphql_subscription;
use sqlx::{Pool, Sqlite};
use url::Url;
use warp::Filter;

use super::schema::build_schema;

pub async fn filter(
    pool: &Pool<Sqlite>,
    external_url: Option<Url>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let schema = build_schema(pool).await.unwrap();
    let graphql_post = warp::path("graphql")
        .and(async_graphql_warp::graphql(schema.clone()))
        .and_then(|(schema, request): (Schema, async_graphql::Request)| async move {
            // Execute query
            let response = schema.execute(request).await;
            // Return result
            Ok::<_, Infallible>(warp::reply::json(&response))
        });

    let subscription_endpoint = if let Some(external_url) = external_url {
        format!("{external_url}/graphql/ws").replace("http", "ws")
    } else {
        "/graphql/ws".to_string()
    };

    let playground_filter = warp::path("graphql").map(move || {
        warp::reply::html(playground_source(
            // NOTE: GraphQL Playground currently doesn't support relative urls for the
            // subscription endpoint.
            GraphQLPlaygroundConfig::new("").subscription_endpoint(subscription_endpoint.as_str()),
        ))
    });

    graphql_subscription(schema).or(graphql_post).or(playground_filter)
}
