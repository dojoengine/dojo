use std::env;
use std::sync::Arc;

use actix_cors::Cors;
use actix_web::http::header;
use actix_web::web::{self, Data};
use actix_web::{middleware, App, Error, HttpResponse, HttpServer};
use juniper::{EmptyMutation, EmptySubscription, RootNode};
use juniper_actix::{graphql_handler, playground_handler};
use sqlx::SqlitePool;

use crate::graphql::Query;

type Schema = RootNode<'static, Query, EmptyMutation<Context>, EmptySubscription<Context>>;

// To make our context usable by Juniper, we have to implement a marker trait.
impl juniper::Context for Context {}

pub struct Context {
    pub schema: Arc<Schema>,
    pub pool: Arc<sqlx::SqlitePool>,
}

#[allow(dead_code)]
fn schema() -> Schema {
    Schema::new(Query, EmptyMutation::<Context>::new(), EmptySubscription::<Context>::new())
}

#[allow(dead_code)]
async fn playground_route() -> Result<HttpResponse, Error> {
    playground_handler("/graphql", None).await
}
#[allow(dead_code)]
async fn graphql_route(
    req: actix_web::HttpRequest,
    payload: actix_web::web::Payload,
    context: web::Data<Context>,
) -> Result<HttpResponse, Error> {
    let schema = context.schema.as_ref();

    graphql_handler(schema, &context, req, payload).await
}

pub async fn start_server(pool: &SqlitePool) -> std::io::Result<()> {
    env::set_var("RUST_LOG", "info");
    env_logger::init();

    let pool = Arc::new(pool.clone());

    let server = HttpServer::new(move || {
        App::new()
            .app_data(Data::new(Context { schema: Arc::new(schema()), pool: pool.clone() }))
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
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();

    start_server(&pool).await
}
