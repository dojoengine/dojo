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

pub async fn start(host: &String, port: u16, pool: &Pool<Sqlite>) -> anyhow::Result<()> {
    let schema = build_schema(pool).await?;

    let app = Route::new()
        .at("/", get(graphiql).post(GraphQL::new(schema.clone())))
        .at("/ws", get(GraphQLSubscription::new(schema)))
        .with(Cors::new());

    println!("Open GraphiQL IDE: http://{}:{}", host, port);
    Server::new(TcpListener::bind(format!("{}:{}", host, port))).run(app).await?;

    Ok(())
}
