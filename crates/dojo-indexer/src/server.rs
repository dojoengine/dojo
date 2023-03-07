use std::env;

use actix_cors::Cors;
use actix_web::http::header;
use actix_web::web::{self, Data};
use actix_web::{middleware, App, Error, HttpResponse, HttpServer};
use juniper::{EmptyMutation, EmptySubscription, RootNode};
use juniper_actix::{graphql_handler, playground_handler};

use crate::graphql::Query;
use crate::prisma::PrismaClient;

// To make our Database usable by Juniper, we have to implement a marker trait.
impl juniper::Context for PrismaClient {}

type Schema =
    RootNode<'static, Query, EmptyMutation<PrismaClient>, EmptySubscription<PrismaClient>>;

#[allow(dead_code)]
fn schema() -> Schema {
    Schema::new(
        Query,
        EmptyMutation::<PrismaClient>::new(),
        EmptySubscription::<PrismaClient>::new(),
    )
}

#[allow(dead_code)]
async fn playground_route() -> Result<HttpResponse, Error> {
    playground_handler("/graphql", None).await
}
#[allow(dead_code)]
async fn graphql_route(
    req: actix_web::HttpRequest,
    payload: actix_web::web::Payload,
    schema: web::Data<Schema>,
) -> Result<HttpResponse, Error> {
    let context = PrismaClient::_builder().build().await.unwrap_or_else(|e| {
        panic!("Failed to connect to database: {}", e);
    });

    graphql_handler(&schema, &context, req, payload).await
}

pub async fn start_server() -> std::io::Result<()> {
    env::set_var("RUST_LOG", "info");
    env_logger::init();

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
