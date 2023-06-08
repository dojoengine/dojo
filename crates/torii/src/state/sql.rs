use std::collections::HashMap;

use anyhow::Result;
use async_trait::async_trait;
use dojo_world::manifest::{Component, Manifest, System};
use sqlx::pool::PoolConnection;
use sqlx::{Executor, Pool, Sqlite};
use starknet::core::types::FieldElement;

use super::{State, World};

#[cfg(test)]
#[path = "sql_test.rs"]
mod test;

pub struct Sql {
    world_address: FieldElement,
    pool: Pool<Sqlite>,
}

impl Sql {
    pub async fn new(pool: Pool<Sqlite>, world_address: FieldElement) -> Result<Self> {
        let queries = vec![
            format!(
                "INSERT OR IGNORE INTO indexers (id, head) VALUES ('{:#x}', '{}')",
                world_address, 0
            ),
            format!(
                "INSERT OR IGNORE INTO worlds (id, world_address) VALUES ('{:#x}', '{:#x}')",
                world_address, world_address
            ),
        ];

        let mut tx = pool.begin().await?;

        for query in queries {
            tx.execute(sqlx::query(&query)).await?;
        }

        tx.commit().await?;

        Ok(Self { pool, world_address })
    }

    async fn execute(&self, queries: Vec<String>) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        for query in queries {
            tx.execute(sqlx::query(&query)).await?;
        }

        tx.commit().await?;

        Ok(())
    }
}

#[async_trait]
impl State for Sql {
    async fn load_from_manifest(&self, manifest: Manifest) -> Result<()> {
        let mut updates = vec![
            format!("world_address = '{:#x}'", self.world_address),
            format!("world_class_hash = '{:#x}'", manifest.world.class_hash),
            format!("executor_class_hash = '{:#x}'", manifest.executor.class_hash),
        ];

        if let Some(executor_address) = manifest.executor.address {
            updates.push(format!("executor_address = '{:#x}'", executor_address));
        }

        let mut queries = vec![];

        queries.push(format!(
            "UPDATE worlds SET {} WHERE id = '{:#x}'",
            updates.join(","),
            self.world_address
        ));

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

        self.execute(queries).await
    }

    async fn head(&self) -> Result<u64> {
        let mut conn: PoolConnection<Sqlite> = self.pool.acquire().await?;
        let indexer: (i64,) = sqlx::query_as(&format!(
            "SELECT head FROM indexers WHERE id = '{:#x}'",
            self.world_address
        ))
        .fetch_one(&mut conn)
        .await?;
        Ok(indexer.0.try_into().expect("doesnt fit in u64"))
    }

    async fn set_head(&self, head: u64) -> Result<()> {
        let mut conn: PoolConnection<Sqlite> = self.pool.acquire().await?;
        sqlx::query(&format!(
            "UPDATE indexers SET head = {head} WHERE id = '{:#x}'",
            self.world_address
        ))
        .execute(&mut conn)
        .await?;
        Ok(())
    }

    async fn world(&self) -> Result<World> {
        let mut conn: PoolConnection<Sqlite> = self.pool.acquire().await?;
        let meta: World =
            sqlx::query_as(&format!("SELECT * FROM worlds WHERE id = '{:#x}'", self.world_address))
                .fetch_one(&mut conn)
                .await?;

        Ok(meta)
    }

    async fn set_world(&self, world: World) -> Result<()> {
        let mut conn: PoolConnection<Sqlite> = self.pool.acquire().await?;
        sqlx::query(&format!(
            "UPDATE worlds SET world_address='{:#x}', world_class_hash='{:#x}', \
             executor_address='{:#x}', executor_class_hash='{:#x}' WHERE id = '{:#x}'",
            world.world_address,
            world.world_class_hash,
            world.executor_address,
            world.executor_class_hash,
            world.world_address,
        ))
        .execute(&mut conn)
        .await?;
        Ok(())
    }

    async fn register_component(&self, component: Component) -> Result<()> {
        let mut queries = vec![build_component_table_create(component.clone())];
        let component_id = component.name.to_lowercase();
        queries.push(format!(
            "INSERT INTO components (id, name, class_hash) VALUES ('{}', '{}', '{:#x}') ON \
             CONFLICT(id) DO UPDATE SET class_hash='{:#x}'",
            component_id, component.name, component.class_hash, component.class_hash
        ));
        self.execute(queries).await
    }

    async fn register_system(&self, system: System) -> Result<()> {
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
        &self,
        component: String,
        partition: FieldElement,
        key: FieldElement,
        members: HashMap<String, FieldElement>,
    ) -> Result<()> {
        let mut columns = vec![];
        let mut values = vec![];
        for (key, value) in members {
            columns.push(format!("external_{}", key));
            values.push(format!("'{value:#x}'"));
        }
        let columns = columns.join(", ");
        let values = values.join(", ");

        let queries = vec![
            format!(
                "INSERT INTO entities (id, partition, keys) VALUES ('{:#x}', '{:#x}', '{}')",
                key, partition, "todo",
            ),
            format!(
                "INSERT INTO external_{} (id, partition, {columns}) VALUES ('{key:#x}', \
                 '{partition:#x}', {values})",
                component.to_lowercase()
            ),
        ];

        self.execute(queries).await
    }

    async fn delete_entity(
        &self,
        component: String,
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
        component: String,
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
        component: String,
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
        "CREATE TABLE IF NOT EXISTS external_{} (id TEXT NOT NULL PRIMARY KEY, partition TEXT NOT \
         NULL, ",
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
