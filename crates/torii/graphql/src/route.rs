use std::convert::Infallible;

use async_graphql::dynamic::Schema;
use async_graphql::http::{playground_source, GraphQLPlaygroundConfig};
use async_graphql_warp::graphql_subscription;
use sqlx::{Pool, Sqlite};
use warp::Filter;

use super::schema::build_schema;

pub async fn filter(
    pool: &Pool<Sqlite>,
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

    let graphiql_filter = warp::path("graphql").map(|| {
        warp::reply::html(playground_source(
            GraphQLPlaygroundConfig::new("/graphql").subscription_endpoint("/graphql/ws"),
        ))
    });

    graphql_subscription(schema).or(graphql_post).or(graphiql_filter)
}
