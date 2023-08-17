use anyhow::Result;
use async_trait::async_trait;
use dojo_world::manifest::{Component, Manifest, System};
use sqlx::pool::PoolConnection;
use sqlx::sqlite::SqliteRow;
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
            "CREATE TABLE IF NOT EXISTS external_{} (entity_id TEXT NOT NULL PRIMARY KEY, ",
            component.name.to_lowercase()
        );

        for member in component.clone().members {
            if member.key {
                continue;
            }

            let sql_type = as_sql_type(&member.ty)?;
            component_table_query.push_str(&format!("external_{} {}, ", member.name, sql_type));
        }

        component_table_query.push_str(
            "created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
        FOREIGN KEY (entity_id) REFERENCES entities(id));",
        );
        queries.push(component_table_query);

        for member in component.members {
            queries.push(format!(
                "INSERT OR IGNORE INTO component_members (component_id, name, type, key) VALUES \
                 ('{}', '{}', '{}', {})",
                component_id, member.name, member.ty, member.key,
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
        keys: Vec<FieldElement>,
        values: Vec<FieldElement>,
    ) -> Result<()> {
        let entity_id = format!("{:#x}", poseidon_hash_many(&keys));
        let entity_result = sqlx::query("SELECT * FROM entities WHERE id = ?")
            .bind(&entity_id)
            .fetch_optional(&self.pool)
            .await?;

        // TODO: map keys to individual columns
        let keys_str = keys.iter().map(|k| format!("{:#x}", k)).collect::<Vec<String>>().join(",");
        let component_names = component_names(entity_result, &component)?;
        let insert_entities = format!(
            "INSERT INTO entities (id, keys, component_names) VALUES ('{}', '{}', '{}') ON \
             CONFLICT(id) DO UPDATE SET
             component_names=excluded.component_names, 
             updated_at=CURRENT_TIMESTAMP",
            entity_id, keys_str, component_names
        );

        let member_results =
            sqlx::query("SELECT * FROM component_members WHERE key == FALSE AND component_id = ?")
                .bind(component.to_lowercase())
                .fetch_all(&self.pool)
                .await?;

        let (names_str, values_str) = format_values(member_results, values)?;
        let insert_components = format!(
            "INSERT OR REPLACE INTO external_{} (entity_id {}) VALUES ('{}' {})",
            component.to_lowercase(),
            names_str,
            entity_id,
            values_str
        );

        // tx commit required
        self.queue(vec![insert_entities, insert_components]).await;
        self.execute().await?;
        Ok(())
    }

    async fn delete_entity(&self, component: String, key: FieldElement) -> Result<()> {
        let query = format!("DELETE FROM {component} WHERE id = {key}");
        self.queue(vec![query]).await;
        Ok(())
    }

    async fn entity(&self, component: String, key: FieldElement) -> Result<Vec<FieldElement>> {
        let query = format!("SELECT * FROM {component} WHERE id = {key}");
        let mut conn: PoolConnection<Sqlite> = self.pool.acquire().await?;
        let row: (i32, String, String) = sqlx::query_as(&query).fetch_one(&mut conn).await?;
        Ok(serde_json::from_str(&row.2).unwrap())
    }

    async fn entities(&self, component: String) -> Result<Vec<Vec<FieldElement>>> {
        let query = format!("SELECT * FROM {component}");
        let mut conn: PoolConnection<Sqlite> = self.pool.acquire().await?;
        let mut rows =
            sqlx::query_as::<_, (i32, String, String)>(&query).fetch_all(&mut conn).await?;
        Ok(rows.drain(..).map(|row| serde_json::from_str(&row.2).unwrap()).collect())
    }
}

fn component_names(entity_result: Option<SqliteRow>, new_component: &str) -> Result<String> {
    let component_names = match entity_result {
        Some(entity) => {
            let existing = entity.try_get::<String, &str>("component_names")?;
            if existing.contains(new_component) {
                existing
            } else {
                format!("{},{}", existing, new_component)
            }
        }
        None => new_component.to_string(),
    };

    Ok(component_names)
}

fn format_values(
    member_results: Vec<SqliteRow>,
    values: Vec<FieldElement>,
) -> Result<(String, String)> {
    let names: Result<Vec<String>> = member_results
        .iter()
        .map(|row| {
            let name = row.try_get::<String, &str>("name")?;
            Ok(format!(",external_{}", name))
        })
        .collect();

    let types: Result<Vec<String>> =
        member_results.iter().map(|row| Ok(row.try_get::<String, &str>("type")?)).collect();

    // format according to type
    let values: Result<Vec<String>> = values
        .iter()
        .zip(types?.iter())
        .map(|(value, ty)| {
            if as_sql_type(ty)? == "INTEGER" {
                Ok(format!(",'{}'", value))
            } else {
                Ok(format!(",'{:#x}'", value))
            }
        })
        .collect();

    Ok((names?.join(""), values?.join("")))
}

fn as_sql_type(s: &str) -> Result<&str, anyhow::Error> {
    match s {
        "u8" => Ok("INTEGER"),
        "u16" => Ok("INTEGER"),
        "u32" => Ok("INTEGER"),
        "u64" => Ok("INTEGER"),
        "u128" => Ok("TEXT"),
        "u256" => Ok("TEXT"),
        "usize" => Ok("INTEGER"),
        "bool" => Ok("INTEGER"),
        "Cursor" => Ok("TEXT"),
        "ContractAddress" => Ok("TEXT"),
        "DateTime" => Ok("TEXT"),
        "felt252" => Ok("TEXT"),
        _ => Err(anyhow::anyhow!("Unknown type {}", s.to_string())),
    }
}
