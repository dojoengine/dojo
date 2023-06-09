use async_graphql::http::GraphiQLSource;
use async_graphql_poem::GraphQL;
use poem::listener::TcpListener;
use poem::web::Html;
use poem::{get, handler, IntoResponse, Route, Server};
use sqlx::{Pool, Sqlite};

use super::schema::build_schema;

#[handler]
async fn graphiql() -> impl IntoResponse {
    Html(GraphiQLSource::build().endpoint("/").finish())
}

pub async fn start_graphql(pool: &Pool<Sqlite>) -> anyhow::Result<()> {
    let schema = build_schema(pool).await?;

    let app = Route::new().at("/", get(graphiql).post(GraphQL::new(schema)));
    Server::new(TcpListener::bind("127.0.0.1:8080")).run(app).await?;

    Ok(())
}
