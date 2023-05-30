use async_graphql::http::{playground_source, GraphQLPlaygroundConfig, GraphiQLSource};
use async_graphql_poem::GraphQL;
use poem::listener::TcpListener;
use poem::web::Html;
use poem::{get, handler, IntoResponse, Route, Server};
use sqlx::{Pool, Sqlite};

use super::schema::build_schema;

#[handler]
async fn graphiql() -> impl IntoResponse {
    Html(GraphiQLSource::build().endpoint("/query").finish())
}

#[handler]
async fn graphql_playground() -> impl IntoResponse {
    Html(playground_source(GraphQLPlaygroundConfig::new("/playground")))
}

pub async fn start_graphql(pool: &Pool<Sqlite>) -> std::io::Result<()> {
    let schema = build_schema(pool).await.expect("failed to build schema");

    let app = Route::new()
        .at("/query", get(graphiql).post(GraphQL::new(schema.clone())))
        .at("/playground", get(graphql_playground).post(GraphQL::new(schema.clone())));
    Server::new(TcpListener::bind("127.0.0.1:8080")).run(app).await?;

    Ok(())
}
