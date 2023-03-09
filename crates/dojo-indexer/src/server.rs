use std::env;

use actix_cors::Cors;
use actix_web::http::header;
use actix_web::web::{self, Data, Json};
use actix_web::{middleware, App, Error, HttpResponse, HttpServer};
use diesel::SqliteConnection;
use diesel::r2d2::Pool;
use juniper::{EmptyMutation};
use juniper_actix::{graphql_handler, playground_handler};
use wundergraph::diesel::r2d2::{ConnectionManager, self, PooledConnection};
use std::sync::Arc;
use wundergraph::scalar::WundergraphScalarValue;
use juniper::http::GraphQLRequest;
use serde::{Deserialize, Serialize};

use crate::schema::{Query, DBConnection, Schema, MyContext};

// actix integration stuff
#[derive(Serialize, Deserialize, Debug)]
pub struct GraphQLData(GraphQLRequest<WundergraphScalarValue>);

#[derive(Clone)]
struct AppState {
    schema: &Schema<MyContext<DBConnection>>,
    pool: &Pool<ConnectionManager<DBConnection>>,
}

#[allow(dead_code)]
fn schema() -> Schema<MyContext<DBConnection>> {
    Schema::new(
        Query::<MyContext<DBConnection>>::default(),
        EmptyMutation::<MyContext<DBConnection>>::new(),
    )
}

#[allow(dead_code)]
async fn playground_route() -> Result<HttpResponse, Error> {
    playground_handler("/graphql", None).await
}

async fn graphql_route(
    req: actix_web::HttpRequest,
    payload: actix_web::web::Payload,
    state: web::Data<AppState>,
) -> Result<HttpResponse, Error> {
    let ctx = MyContext::new(state.get_ref().pool.get().unwrap());
    // let res = data.execute(&state.get_ref().schema, &ctx);
    // let json = serde_json::to_string(&res)?;
    graphql_handler(state.get_ref().schema, &ctx, req, payload).await
    // Ok(HttpResponse::Ok()
        // .content_type("application/json")
        // .body(json))
}

pub async fn start_server() -> std::io::Result<()> {
    env::set_var("RUST_LOG", "info");
    env_logger::init();

    let manager = ConnectionManager::<SqliteConnection>::new("indexer.db");
    let pool = r2d2::Pool::builder().build(manager).expect("Failed to create pool.");

    let server = HttpServer::new(move || {
        App::new()
            .app_data(Data::new(schema()))
            .wrap(
                Cors::default()
                    .allow_any_origin()
                    .allowed_methods(vec!["POST", "GET"])
                    .allowed_headers(vec![header::AUTHORIZATION, header::ACCEPT])
                    .allowed_header(header::CONTENT_TYPE)
                    .supports_credentials()
                    .max_age(3600),
            )
            .wrap(middleware::Compress::default())
            .wrap(middleware::Logger::default())
            .service(
                web::resource("/graphql")
                    .route(web::post().to(graphql_route))
                    .route(web::get().to(graphql_route)),
            )
            .service(web::resource("/playground").route(web::get().to(playground_route)))
    });
    server.bind("127.0.0.1:8080").unwrap().run().await
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    start_server().await
}
