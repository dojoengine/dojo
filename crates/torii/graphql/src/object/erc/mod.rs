use async_graphql::Value;

use super::connection::cursor;
use crate::query::order::CursorDirection;

pub mod erc_token;
pub mod token_balance;
pub mod token_transfer;

fn handle_cursor(
    cursor: &str,
    direction: CursorDirection,
    id_column: &str,
) -> sqlx::Result<String> {
    match cursor::decode(cursor) {
        Ok((event_id, _)) => Ok(format!("{} {} '{}'", id_column, direction.as_ref(), event_id)),
        Err(_) => Err(sqlx::Error::Decode("Invalid cursor format".into())),
    }
}

#[derive(Debug, Clone)]
pub struct ConnectionEdge<T> {
    pub node: T,
    pub cursor: String,
}

#[derive(Debug, Clone)]
pub struct Connection<T> {
    pub total_count: i64,
    pub edges: Vec<ConnectionEdge<T>>,
    pub page_info: Value,
}
