use anyhow::Result;
use async_trait::async_trait;
use sqlx::{pool::PoolConnection, Pool, Sqlite};
use starknet::core::types::FieldElement;

use super::Storage;

pub struct SqlStorage {
    pool: Pool<Sqlite>,
}

impl SqlStorage {
    pub fn new(pool: Pool<Sqlite>) -> Result<Self> {
        Ok(Self { pool })
    }
}

#[async_trait]
impl Storage for SqlStorage {
    async fn head(&self) -> Result<u64> {
        let mut conn: PoolConnection<Sqlite> = self.pool.acquire().await?;
        let indexer: (i64,) =
            sqlx::query_as("SELECT head FROM indexer WHERE id = 0").fetch_one(&mut conn).await?;

        Ok(indexer.0.try_into().expect("doesnt fit in u64"))
    }

    async fn set_head(&mut self, head: u64) -> Result<()> {
        let mut conn: PoolConnection<Sqlite> = self.pool.acquire().await?;
        sqlx::query(&format!("INSERT INTO indexer (head) WHERE id = {head}"))
            .execute(&mut conn)
            .await?;
        Ok(())
    }

    async fn create_component(&self, name: FieldElement, columns: Vec<FieldElement>) -> Result<()> {
        let mut query = format!(
            "CREATE TABLE IF NOT EXISTS {} (id SERIAL PRIMARY KEY, partition TEXT NOT NULL, ",
            name
        );
        for column in columns {
            query.push_str(&format!("{} TEXT, ", column));
        }
        query.push_str(");");
        let mut conn: PoolConnection<Sqlite> = self.pool.acquire().await?;
        sqlx::query(&query).execute(&mut conn).await?;
        Ok(())
    }

    async fn set_entity(
        &self,
        component: FieldElement,
        partition: FieldElement,
        key: FieldElement,
        values: Vec<FieldElement>,
    ) -> Result<()> {
        let mut query = format!("INSERT INTO {} (id, partition", component);
        for i in 0..values.len() {
            query.push_str(&format!(", column{}", i + 1));
        }
        query.push_str(&format!(") VALUES ({key}, {partition}"));
        for value in values.iter() {
            query.push_str(&format!(", {value}"));
        }
        query.push_str(");");
        let mut conn = self.pool.acquire().await?;
        sqlx::query(&query).execute(&mut conn).await?;
        Ok(())
    }

    async fn delete_entity(
        &self,
        component: FieldElement,
        partition: FieldElement,
        key: FieldElement,
    ) -> Result<()> {
        let query = format!("DELETE FROM {component} WHERE id = {key} AND partition = {partition}");
        let mut conn: PoolConnection<Sqlite> = self.pool.acquire().await?;
        sqlx::query(&query).execute(&mut conn).await?;
        Ok(())
    }

    async fn entity(
        &self,
        component: FieldElement,
        partition: FieldElement,
        key: FieldElement,
    ) -> Result<Vec<FieldElement>> {
        let query =
            format!("SELECT * FROM {component} WHERE id = {key} AND partition = {partition}");
        let mut conn: PoolConnection<Sqlite> = self.pool.acquire().await?;
        let row: (i32, String, String) = sqlx::query_as(&query).fetch_one(&mut conn).await?;
        Ok(serde_json::from_str(&row.2).unwrap())
    }

    async fn entities(
        &self,
        component: FieldElement,
        partition: FieldElement,
    ) -> Result<Vec<Vec<FieldElement>>> {
        let query = format!("SELECT * FROM {component} WHERE partition = {partition}");
        let mut conn: PoolConnection<Sqlite> = self.pool.acquire().await?;
        let mut rows =
            sqlx::query_as::<_, (i32, String, String)>(&query).fetch_all(&mut conn).await?;
        Ok(rows.drain(..).map(|row| serde_json::from_str(&row.2).unwrap()).collect())
    }
}
