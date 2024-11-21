use std::future::Future;
use std::net::SocketAddr;

use async_graphql::dynamic::Schema;
use async_graphql::http::GraphiQLSource;
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

    let (graphql_endpoint, subscription_endpoint) = if let Some(external_url) = external_url {
        let mut graphql_url = external_url;
        graphql_url.set_path("graphql");

        let mut websocket_url = graphql_url.clone();
        websocket_url.set_path("ws");
        let _ = websocket_url.set_scheme(match websocket_url.scheme() {
            "https" => "wss",
            "http" => "ws",
            _ => panic!("Invalid URL scheme - must be http or https"),
        });

        (graphql_url.path().to_string(), websocket_url.to_string())
    } else {
        ("graphql".to_string(), "graphql/ws".to_string())
    };

    let playground_filter = warp::path("graphql").map(move || {
        warp::reply::html(
            GraphiQLSource::build()
                .endpoint(&graphql_endpoint)
                .subscription_endpoint(&subscription_endpoint)
                .finish(),
        )
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
