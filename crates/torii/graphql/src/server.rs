use async_graphql::http::GraphiQLSource;
use async_graphql_poem::{GraphQL, GraphQLSubscription};
use poem::listener::TcpListener;
use poem::middleware::Cors;
use poem::web::Html;
use poem::{get, handler, EndpointExt, IntoResponse, Route, Server};
use sqlx::{Pool, Sqlite};

use super::schema::build_schema;

#[handler]
async fn graphiql() -> impl IntoResponse {
    Html(GraphiQLSource::build().endpoint("/").subscription_endpoint("/ws").finish())
}

pub async fn start(pool: Pool<Sqlite>) -> anyhow::Result<()> {
    let schema = build_schema(&pool).await?;

    let app = Route::new()
        .at("/", get(graphiql).post(GraphQL::new(schema.clone())))
        .at("/ws", get(GraphQLSubscription::new(schema)))
        .with(Cors::new());

    println!("Open GraphiQL IDE: http://localhost:8080");
    Server::new(TcpListener::bind("0.0.0.0:8080")).run(app).await?;

    Ok(())
}
