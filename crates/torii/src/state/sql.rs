use anyhow::Result;
use async_trait::async_trait;
use dojo_world::manifest::{Component, Manifest, System};
use sqlx::pool::PoolConnection;
use sqlx::{Executor, Pool, Row, Sqlite};
use starknet::core::types::FieldElement;
use starknet_crypto::poseidon_hash_many;
use tokio::sync::Mutex;

use super::{State, World};

#[cfg(test)]
#[path = "sql_test.rs"]
mod test;

#[async_trait]
pub trait Executable {
    async fn execute(&self) -> Result<()>;
    async fn queue(&self, queries: Vec<String>);
}

pub struct Sql {
    world_address: FieldElement,
    pool: Pool<Sqlite>,
    query_queue: Mutex<Vec<String>>,
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

        Ok(Self { pool, world_address, query_queue: Mutex::new(vec![]) })
    }
}

#[async_trait]
impl Executable for Sql {
    async fn queue(&self, queries: Vec<String>) {
        let mut query_queue = self.query_queue.lock().await;
        query_queue.extend(queries);
    }

    async fn execute(&self) -> Result<()> {
        let queries;
        {
            let mut query_queue = self.query_queue.lock().await;
            queries = query_queue.clone();
            query_queue.clear();
        }

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

        self.queue(vec![format!(
            "UPDATE worlds SET {} WHERE id = '{:#x}'",
            updates.join(","),
            self.world_address
        )])
        .await;

        for component in manifest.components {
            self.register_component(component).await?;
        }

        for system in manifest.systems {
            self.register_system(system).await?;
        }

        self.execute().await
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
        self.queue(vec![format!(
            "UPDATE indexers SET head = {head} WHERE id = '{:#x}'",
            self.world_address
        )])
        .await;
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
        self.queue(vec![format!(
            "UPDATE worlds SET world_address='{:#x}', world_class_hash='{:#x}', \
             executor_address='{:#x}', executor_class_hash='{:#x}' WHERE id = '{:#x}'",
            world.world_address,
            world.world_class_hash,
            world.executor_address,
            world.executor_class_hash,
            world.world_address,
        )])
        .await;
        Ok(())
    }

    async fn register_component(&self, component: Component) -> Result<()> {
        let component_id = component.name.to_lowercase();
        let mut queries = vec![format!(
            "INSERT INTO components (id, name, class_hash) VALUES ('{}', '{}', '{:#x}') ON \
             CONFLICT(id) DO UPDATE SET class_hash='{:#x}'",
            component_id, component.name, component.class_hash, component.class_hash
        )];

        let mut component_table_query = format!(
            "CREATE TABLE IF NOT EXISTS external_{} (id TEXT NOT NULL PRIMARY KEY, partition TEXT \
             NOT NULL, ",
            component.name.to_lowercase()
        );

        // TODO: Set type based on member type
        for member in component.clone().members {
            component_table_query.push_str(&format!("external_{} TEXT, ", member.name));
        }

        component_table_query.push_str(
            "created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
        FOREIGN KEY (id) REFERENCES entities(id));",
        );
        queries.push(component_table_query);

        for member in component.members {
            queries.push(format!(
                "INSERT OR IGNORE INTO component_members (component_id, name, type, slot, offset) \
                 VALUES ('{}', '{}', '{}', '{}', '{}')",
                component_id, member.name, member.ty, member.slot, member.offset,
            ));
        }

        self.queue(queries).await;
        Ok(())
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
        self.queue(vec![query]).await;
        Ok(())
    }

    async fn set_entity(
        &self,
        component: String,
        partition: FieldElement,
        keys: Vec<FieldElement>,
        values: Vec<FieldElement>,
    ) -> Result<()> {
        let results =
            sqlx::query("SELECT * FROM component_members WHERE component_id = ? ORDER BY slot")
                .bind(component.to_lowercase())
                .fetch_all(&self.pool)
                .await?;

        let columns = results
            .iter()
            .map(|row| {
                let name = row.try_get::<String, &str>("name").expect("no name column");
                format!("external_{}", name)
            })
            .collect::<Vec<String>>()
            .join(", ");

        let values =
            values.iter().map(|v| format!("'{:#x}'", v)).collect::<Vec<String>>().join(", ");
        let entity_id = poseidon_hash_many(&keys);
        let keys_str = keys.iter().map(|k| format!("{:#x}", k)).collect::<Vec<String>>().join(",");

        let queries = vec![
            format!(
                "INSERT OR REPLACE INTO entities (id, partition, keys) VALUES ('{:#x}', '{:#x}', \
                 '{}')",
                entity_id, partition, keys_str,
            ),
            format!(
                "INSERT OR REPLACE INTO external_{} (id, partition, {columns}) VALUES \
                 ('{entity_id:#x}', '{partition:#x}', {values})",
                component.to_lowercase()
            ),
        ];

        self.queue(queries).await;
        Ok(())
    }

    async fn delete_entity(
        &self,
        component: String,
        partition: FieldElement,
        key: FieldElement,
    ) -> Result<()> {
        let query = format!("DELETE FROM {component} WHERE id = {key} AND partition = {partition}");
        self.queue(vec![query]).await;
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
