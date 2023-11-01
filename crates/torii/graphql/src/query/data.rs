use sqlx::pool::PoolConnection;
use sqlx::sqlite::SqliteRow;
use sqlx::{Result, Sqlite};

use super::filter::{Filter, FilterValue};
use super::order::{CursorDirection, Direction, Order};
use crate::constants::DEFAULT_LIMIT;
use crate::object::connection::{cursor, ConnectionArguments};

pub async fn count_rows(
    conn: &mut PoolConnection<Sqlite>,
    table_name: &str,
    keys: &Option<Vec<String>>,
    filters: &Option<Vec<Filter>>,
) -> Result<i64> {
    let mut query = format!("SELECT COUNT(*) FROM {}", table_name);
    let conditions = build_conditions(keys, filters);

    if !conditions.is_empty() {
        query.push_str(&format!(" WHERE {}", conditions.join(" AND ")));
    }

    let result: (i64,) = sqlx::query_as(&query).fetch_one(conn).await?;
    Ok(result.0)
}

pub async fn fetch_single_row(
    conn: &mut PoolConnection<Sqlite>,
    table_name: &str,
    id_column: &str,
    id: &str,
) -> sqlx::Result<SqliteRow> {
    let query = format!("SELECT * FROM {} WHERE {} = '{}'", table_name, id_column, id);
    sqlx::query(&query).fetch_one(conn).await
}

pub async fn fetch_multiple_rows(
    conn: &mut PoolConnection<Sqlite>,
    table_name: &str,
    id_column: &str,
    keys: &Option<Vec<String>>,
    order: &Option<Order>,
    filters: &Option<Vec<Filter>>,
    connection: &ConnectionArguments,
) -> Result<Vec<SqliteRow>> {
    let mut conditions = build_conditions(keys, filters);

    if let Some(after_cursor) = &connection.after {
        conditions.push(handle_cursor(after_cursor, order, CursorDirection::After, id_column)?);
    }

    if let Some(before_cursor) = &connection.before {
        conditions.push(handle_cursor(before_cursor, order, CursorDirection::Before, id_column)?);
    }

    let mut query = format!("SELECT * FROM {}", table_name);
    if !conditions.is_empty() {
        query.push_str(&format!(" WHERE {}", conditions.join(" AND ")));
    }

    let limit = connection.first.or(connection.last).or(connection.limit).unwrap_or(DEFAULT_LIMIT);

    // NOTE: Order is determined by the `order` param if provided, otherwise it's inferred from the
    // `first` or `last` param. Explicit ordering take precedence
    match order {
        Some(order) => {
            let column_name = format!("external_{}", order.field);
            query.push_str(&format!(
                " ORDER BY {column_name} {}, {id_column} {} LIMIT {limit}",
                order.direction.as_ref(),
                order.direction.as_ref()
            ));
        }
        None => {
            let order_direction = match (connection.first, connection.last) {
                (Some(_), _) => Direction::Desc,
                (_, Some(_)) => Direction::Asc,
                _ => Direction::Desc,
            };

            query.push_str(&format!(
                " ORDER BY {id_column} {} LIMIT {limit}",
                order_direction.as_ref()
            ));
        }
    };

    if let Some(offset) = connection.offset {
        query.push_str(&format!(" OFFSET {}", offset));
    }

    sqlx::query(&query).fetch_all(conn).await
}

fn handle_cursor(
    cursor: &str,
    order: &Option<Order>,
    direction: CursorDirection,
    id_column: &str,
) -> Result<String> {
    match cursor::decode(cursor) {
        Ok((event_id, field_value)) => match order {
            Some(order) => {
                let field_name = format!("external_{}", order.field);
                Ok(format!(
                    "({} {} '{}' AND {} = '{}') OR {} {} '{}'",
                    id_column,
                    direction.as_ref(),
                    event_id,
                    field_name,
                    field_value,
                    field_name,
                    direction.as_ref(),
                    field_value
                ))
            }
            None => Ok(format!("{} {} '{}'", id_column, direction.as_ref(), event_id)),
        },
        Err(_) => Err(sqlx::Error::Decode("Invalid cursor format".into())),
    }
}

fn build_conditions(keys: &Option<Vec<String>>, filters: &Option<Vec<Filter>>) -> Vec<String> {
    let mut conditions = Vec::new();

    if let Some(keys) = &keys {
        let keys_str = keys.join("/").replace('*', "%");
        conditions.push(format!("keys LIKE '{}/%'", keys_str));
    }

    if let Some(filters) = filters {
        conditions.extend(filters.iter().map(|filter| match &filter.value {
            FilterValue::Int(i) => format!("{} {} {}", filter.field, filter.comparator, i),
            FilterValue::String(s) => format!("{} {} '{}'", filter.field, filter.comparator, s),
        }));
    }

    conditions
}
