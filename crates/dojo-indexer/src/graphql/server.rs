use actix_cors::Cors;
use actix_web::http::header;
use actix_web::web::{self, Data};
use actix_web::{guard, middleware, App, HttpResponse, HttpServer, Result};
use async_graphql::http::GraphiQLSource;
use async_graphql::{EmptyMutation, EmptySubscription, Schema};
use async_graphql_actix_web::{GraphQLRequest, GraphQLResponse};
use sqlx::SqlitePool;
use tracing::info;

use super::Query;

pub type IndexerSchema = Schema<Query, EmptyMutation, EmptySubscription>;

async fn index(schema: web::Data<IndexerSchema>, req: GraphQLRequest) -> GraphQLResponse {
    schema.execute(req.into_inner()).await.into()
}

async fn index_graphiql() -> Result<HttpResponse> {
    Ok(HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(GraphiQLSource::build().endpoint("/query").finish()))
}

pub async fn start_graphql(pool: &SqlitePool) -> std::io::Result<()> {
    info!(target: "starting graphql server", addr = "127.0.0.1:8080");

    let schema = Schema::build(Query, EmptyMutation, EmptySubscription).data(pool.clone()).finish();

    let server = HttpServer::new(move || {
        App::new()
            .app_data(Data::new(schema.clone()))
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
            .service(web::resource("/query").guard(guard::Post()).to(index))
            .service(web::resource("/playground").guard(guard::Get()).to(index_graphiql))
    });
    server.bind("127.0.0.1:8080").unwrap().run().await
}
