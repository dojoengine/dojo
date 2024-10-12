use super::connection::cursor;
use crate::query::order::CursorDirection;

pub mod erc_balance;
pub mod erc_token;
pub mod erc_transfer;

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
