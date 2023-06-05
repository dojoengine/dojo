use anyhow::Result;
use async_trait::async_trait;
use dojo_world::manifest::{Component, Manifest, System};
use sqlx::pool::PoolConnection;
use sqlx::{Executor, Pool, Sqlite};
use starknet::core::types::FieldElement;

use super::State;

#[cfg(test)]
#[path = "sql_test.rs"]
mod test;

pub struct Sql {
    pool: Pool<Sqlite>,
}

impl Sql {
    pub fn new(pool: Pool<Sqlite>) -> Result<Self> {
        Ok(Self { pool })
    }
}

#[async_trait]
impl State for Sql {
    async fn load_from_manifest(&mut self, manifest: Manifest) -> Result<()> {
        let mut updates = vec![
            format!("world_class_hash = '{:#x}'", manifest.world.class_hash),
            format!("executor_class_hash = '{:#x}'", manifest.executor.class_hash),
        ];

        if let Some(world_address) = manifest.world.address {
            updates.push(format!("world_address = '{:#x}'", world_address));
        }

        if let Some(executor_address) = manifest.executor.address {
            updates.push(format!("executor_address = '{:#x}'", executor_address));
        }

        let mut queries = vec![];

        queries.push(format!("UPDATE indexer SET {} WHERE id = 0", updates.join(",")));

        for component in manifest.components {
            let component_id = component.name.to_lowercase();
            queries.push(format!(
                "INSERT INTO components (id, name, class_hash) VALUES ('{}', '{}', '{:#x}') ON \
                 CONFLICT(id) DO UPDATE SET class_hash='{:#x}'",
                component_id, component.name, component.class_hash, component.class_hash
            ));

            queries.push(build_component_table_create(component.clone()));

            for member in component.members {
                queries.push(format!(
                    "INSERT OR IGNORE INTO component_members (component_id, name, type, slot, \
                     offset) VALUES ('{}', '{}', '{}', '{}', '{}')",
                    component_id, member.name, member.ty, member.slot, member.offset,
                ));
            }
        }

        for system in manifest.systems {
            queries.push(format!(
                "INSERT INTO systems (id, name, class_hash) VALUES ('{}', '{}', '{:#x}') ON \
                 CONFLICT(id) DO UPDATE SET class_hash='{:#x}'",
                system.name.to_lowercase(),
                system.name,
                system.class_hash,
                system.class_hash
            ));
        }

        let mut tx = self.pool.begin().await?;

        for query in queries {
            tx.execute(sqlx::query(&query)).await?;
        }

        tx.commit().await?;

        Ok(())
    }

    async fn head(&self) -> Result<u64> {
        let mut conn: PoolConnection<Sqlite> = self.pool.acquire().await?;
        let indexer: (i64,) =
            sqlx::query_as("SELECT head FROM indexer WHERE id = 1").fetch_one(&mut conn).await?;

        Ok(indexer.0.try_into().expect("doesnt fit in u64"))
    }

    async fn set_head(&mut self, head: u64) -> Result<()> {
        let mut conn: PoolConnection<Sqlite> = self.pool.acquire().await?;
        sqlx::query(&format!("INSERT INTO indexer (head) VALUES ({head}) WHERE id = 0"))
            .execute(&mut conn)
            .await?;
        Ok(())
    }

    async fn register_component(&mut self, component: Component) -> Result<()> {
        let query = build_component_table_create(component);
        let mut conn: PoolConnection<Sqlite> = self.pool.acquire().await?;
        sqlx::query(&query).execute(&mut conn).await?;
        Ok(())
    }

    async fn register_system(&mut self, system: System) -> Result<()> {
        let query = format!(
            "INSERT INTO systems (id, name, class_hash) VALUES ('{}', '{}', '{:#x}') ON \
             CONFLICT(id) DO UPDATE SET class_hash='{:#x}'",
            system.name.to_lowercase(),
            system.name,
            system.class_hash,
            system.class_hash
        );
        let mut conn: PoolConnection<Sqlite> = self.pool.acquire().await?;
        sqlx::query(&query).execute(&mut conn).await?;
        Ok(())
    }

    async fn set_entity(
        &mut self,
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
        &mut self,
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

fn build_component_table_create(component: Component) -> String {
    let mut query = format!(
        "CREATE TABLE IF NOT EXISTS {} (id TEXT NOT NULL PRIMARY KEY, partition TEXT NOT NULL, ",
        component.name.to_lowercase()
    );

    // TODO: Set type based on member type
    for member in component.members {
        query.push_str(&format!("external_{} TEXT, ", member.name));
    }

    query.push_str(
        "
        created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
        FOREIGN KEY (id) REFERENCES entities(id));",
    );
    query
}
