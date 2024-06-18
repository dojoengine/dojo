use std::future::Future;
use std::net::SocketAddr;

use async_graphql::dynamic::Schema;
use async_graphql::http::{playground_source, GraphQLPlaygroundConfig};
use async_graphql::Request;
use async_graphql_warp::graphql_subscription;
use serde_json::json;
use sqlx::{Pool, Sqlite};
use tokio::sync::broadcast::Receiver;
use url::Url;
use warp::{Filter, Rejection, Reply};

use super::schema::build_schema;
use crate::constants::MODEL_TABLE;
use crate::query::data::count_rows;

pub async fn new(
    mut shutdown_rx: Receiver<()>,
    pool: &Pool<Sqlite>,
    external_url: Option<Url>,
) -> (SocketAddr, impl Future<Output = ()> + 'static) {
    let schema = build_schema(pool).await.unwrap();
    let mut conn = pool.acquire().await.unwrap();
    let num_models = count_rows(&mut conn, MODEL_TABLE, &None, &None).await.unwrap();

    let routes = graphql_filter(schema, external_url, num_models == 0);
    warp::serve(routes).bind_with_graceful_shutdown(([127, 0, 0, 1], 0), async move {
        shutdown_rx.recv().await.ok();
    })
}

fn graphql_filter(
    schema: Schema,
    external_url: Option<Url>,
    is_empty: bool,
) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    let graphql_post = async_graphql_warp::graphql(schema.clone()).and_then(
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
        let mut websocket_url = external_url.clone();
        websocket_url.set_path("/graphql/ws");

        let websocket_scheme = match websocket_url.scheme() {
            "http" => "ws",
            "https" => "wss",
            _ => panic!("Invalid URL scheme"), // URL validated on input so this never hits
        };

        let _ = websocket_url.set_scheme(websocket_scheme);
        websocket_url.to_string()
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
