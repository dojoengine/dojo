use async_graphql::dynamic::Schema;
use async_graphql::http::{playground_source, GraphQLPlaygroundConfig};
use async_graphql::Request;
use async_graphql_warp::{graphql, graphql_subscription};
use serde_json::json;
use sqlx::{Pool, Sqlite};
use url::Url;
use warp::{Filter, Rejection, Reply};

use super::schema::build_schema;
use crate::query::constants::MODEL_TABLE;
use crate::query::data::count_rows;

pub async fn filter(
    pool: &Pool<Sqlite>,
    external_url: Option<Url>,
) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    let schema = build_schema(pool).await.unwrap();
    let mut conn = pool.acquire().await.unwrap();
    let num_models = count_rows(&mut conn, MODEL_TABLE, &None, &None).await.unwrap();

    graphql_filter(schema, external_url, num_models == 0)
}

fn graphql_filter(
    schema: Schema,
    external_url: Option<Url>,
    is_empty: bool,
) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    let graphql_post = warp::path("graphql").and(graphql(schema.clone())).and_then(
        move |(schema, request): (Schema, Request)| async move {
            if is_empty {
                return Ok::<_, Rejection>(empty_response());
            }

            // Execute query
            let response = schema.execute(request).await;
            // Return result
            Ok::<_, Rejection>(warp::reply::json(&response))
        },
    );

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

fn empty_response() -> warp::reply::Json {
    let empty_response = json!({
        "errors": [{
            "message": "World does not have any indexed data yet."
        }]
    });
    warp::reply::json(&empty_response)
}
