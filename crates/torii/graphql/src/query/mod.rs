use sqlx::pool::PoolConnection;
use sqlx::sqlite::SqliteRow;
use sqlx::{FromRow, QueryBuilder, Result, Sqlite};

use self::filter::{Filter, FilterValue};

pub mod filter;
pub mod order;

pub enum ID {
    Str(String),
    I64(i64),
}

pub async fn query_by_id<T>(
    conn: &mut PoolConnection<Sqlite>,
    table_name: &str,
    id: ID,
) -> Result<T>
where
    T: Send + Unpin + for<'a> FromRow<'a, SqliteRow>,
{
    let query = format!("SELECT * FROM {} WHERE id = ?", table_name);
    let result = match id {
        ID::Str(id) => sqlx::query_as::<_, T>(&query).bind(id).fetch_one(conn).await?,
        ID::I64(id) => sqlx::query_as::<_, T>(&query).bind(id).fetch_one(conn).await?,
    };
    Ok(result)
}

pub async fn query_all<T>(
    conn: &mut PoolConnection<Sqlite>,
    table_name: &str,
    limit: i64,
) -> Result<Vec<T>>
where
    T: Send + Unpin + for<'a> FromRow<'a, SqliteRow>,
{
    let mut builder: QueryBuilder<'_, Sqlite> = QueryBuilder::new("SELECT * FROM ");
    builder.push(table_name).push(" ORDER BY created_at DESC LIMIT ").push(limit);
    let results: Vec<T> = builder.build_query_as().fetch_all(conn).await?;
    Ok(results)
}

pub async fn query_total_count(
    conn: &mut PoolConnection<Sqlite>,
    table_name: &str,
    filters: &Vec<Filter>,
) -> Result<i64> {
    let mut query = format!("SELECT COUNT(*) FROM {}", table_name);
    let mut conditions = Vec::new();

    for filter in filters {
        let condition = match filter.value {
            FilterValue::Int(i) => format!("{} {} {}", filter.field, filter.comparator, i),
            FilterValue::String(ref s) => format!("{} {} '{}'", filter.field, filter.comparator, s),
        };

        conditions.push(condition);
    }

    if !conditions.is_empty() {
        query.push_str(&format!(" WHERE {}", conditions.join(" AND ")));
    }

    let result: (i64,) = sqlx::query_as(&query).fetch_one(conn).await?;
    Ok(result.0)
}
