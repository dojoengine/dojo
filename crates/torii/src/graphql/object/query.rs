use sqlx::pool::PoolConnection;
use sqlx::sqlite::SqliteRow;
use sqlx::{FromRow, QueryBuilder, Result, Sqlite};

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
    limit: u64,
) -> Result<Vec<T>>
where
    T: Send + Unpin + for<'a> FromRow<'a, SqliteRow>,
{
    let mut builder: QueryBuilder<'_, Sqlite> = QueryBuilder::new("SELECT * FROM ");
    builder.push(table_name).push(" ORDER BY created_at DESC LIMIT ").push(limit);
    let results: Vec<T> = builder.build_query_as().fetch_all(conn).await?;
    Ok(results)
}
