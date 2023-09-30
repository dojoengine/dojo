use std::str::FromStr;

use anyhow::Result;
use chrono::{DateTime, Utc};
use dojo_types::core::CairoType;
use dojo_types::schema::Ty;
use dojo_world::manifest::{Manifest, System};
use sqlx::pool::PoolConnection;
use sqlx::sqlite::SqliteRow;
use sqlx::{Executor, Pool, Row, Sqlite};
use starknet::core::types::{Event, FieldElement};
use starknet_crypto::poseidon_hash_many;

use super::World;
use crate::simple_broker::SimpleBroker;
use crate::types::{Entity, Model as ModelType};

#[cfg(test)]
#[path = "sql_test.rs"]
mod test;

pub struct Sql {
    world_address: FieldElement,
    pool: Pool<Sqlite>,
    query_queue: Vec<String>,
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

        Ok(Self { pool, world_address, query_queue: vec![] })
    }

    pub async fn load_from_manifest(&mut self, manifest: Manifest) -> Result<()> {
        let mut updates = vec![
            format!("world_address = '{:#x}'", self.world_address),
            format!("world_class_hash = '{:#x}'", manifest.world.class_hash),
            format!("executor_class_hash = '{:#x}'", manifest.executor.class_hash),
        ];

        if let Some(executor_address) = manifest.executor.address {
            updates.push(format!("executor_address = '{:#x}'", executor_address));
        }

        self.query_queue.push(format!(
            "UPDATE worlds SET {} WHERE id = '{:#x}'",
            updates.join(","),
            self.world_address
        ));

        for system in manifest.systems {
            self.register_system(system).await?;
        }

        self.execute().await
    }

    pub async fn head(&self) -> Result<u64> {
        let mut conn: PoolConnection<Sqlite> = self.pool.acquire().await?;
        let indexer: (i64,) = sqlx::query_as(&format!(
            "SELECT head FROM indexers WHERE id = '{:#x}'",
            self.world_address
        ))
        .fetch_one(&mut conn)
        .await?;
        Ok(indexer.0.try_into().expect("doesnt fit in u64"))
    }

    pub async fn set_head(&mut self, head: u64) -> Result<()> {
        self.query_queue.push(format!(
            "UPDATE indexers SET head = {head} WHERE id = '{:#x}'",
            self.world_address
        ));
        Ok(())
    }

    pub async fn world(&self) -> Result<World> {
        let mut conn: PoolConnection<Sqlite> = self.pool.acquire().await?;
        let meta: World =
            sqlx::query_as(&format!("SELECT * FROM worlds WHERE id = '{:#x}'", self.world_address))
                .fetch_one(&mut conn)
                .await?;

        Ok(meta)
    }

    pub async fn set_world(&mut self, world: World) -> Result<()> {
        self.query_queue.push(format!(
            "UPDATE worlds SET world_address='{:#x}', world_class_hash='{:#x}', \
             executor_address='{:#x}', executor_class_hash='{:#x}' WHERE id = '{:#x}'",
            world.world_address,
            world.world_class_hash,
            world.executor_address,
            world.executor_class_hash,
            world.world_address,
        ));
        Ok(())
    }

    pub async fn register_model(
        &mut self,
        model: Ty,
        layout: Vec<FieldElement>,
        class_hash: FieldElement,
    ) -> Result<()> {
        let layout_blob = layout.iter().map(|x| (*x).try_into().unwrap()).collect::<Vec<u8>>();
        self.query_queue.push(format!(
            "INSERT INTO models (id, name, class_hash, layout) VALUES ('{}', '{}', '{:#x}', '{}') \
             ON CONFLICT(id) DO UPDATE SET class_hash='{:#x}'",
            model.name(),
            model.name(),
            class_hash,
            hex::encode(&layout_blob),
            class_hash
        ));

        let mut model_idx = 0_usize;
        self.build_queries_recursive(&model, vec![model.name()], &mut model_idx);

        // Since previous query has not been executed, we have to make sure created_at exists
        let created_at: DateTime<Utc> =
            match sqlx::query("SELECT created_at FROM models WHERE id = ?")
                .bind(model.name())
                .fetch_one(&self.pool)
                .await
            {
                Ok(query_result) => query_result.try_get("created_at")?,
                Err(_) => Utc::now(),
            };

        SimpleBroker::publish(ModelType {
            id: model.name(),
            name: model.name(),
            class_hash: format!("{:#x}", class_hash),
            transaction_hash: "0x0".to_string(),
            created_at,
        });
        Ok(())
    }

    pub async fn register_system(&mut self, system: System) -> Result<()> {
        let query = format!(
            "INSERT INTO systems (id, name, class_hash) VALUES ('{}', '{}', '{:#x}') ON \
             CONFLICT(id) DO UPDATE SET class_hash='{:#x}'",
            system.name, system.name, system.class_hash, system.class_hash
        );
        self.query_queue.push(query);
        Ok(())
    }

    pub async fn set_entity(
        &mut self,
        model: String,
        keys: Vec<FieldElement>,
        values: Vec<FieldElement>,
    ) -> Result<()> {
        let entity_id = format!("{:#x}", poseidon_hash_many(&keys));
        let entity_result = sqlx::query("SELECT * FROM entities WHERE id = ?")
            .bind(&entity_id)
            .fetch_optional(&self.pool)
            .await?;

        let keys_str = felts_sql_string(&keys);
        let model_names = model_names_sql_string(entity_result, &model)?;
        self.query_queue.push(format!(
            "INSERT INTO entities (id, keys, model_names) VALUES ('{}', '{}', '{}') ON \
             CONFLICT(id) DO UPDATE SET model_names=excluded.model_names, \
             updated_at=CURRENT_TIMESTAMP",
            entity_id, keys_str, model_names
        ));

        let members: Vec<(String, String, String)> = sqlx::query_as(
            "SELECT id, name, type FROM model_members WHERE model_id = ? ORDER BY model_idx, \
             member_idx ASC",
        )
        .bind(model.clone())
        .fetch_all(&self.pool)
        .await?;

        let (primitive_members, _): (Vec<_>, Vec<_>) =
            members.into_iter().partition(|member| CairoType::from_str(&member.2).is_ok());

        // keys are part of model members, so combine keys and model values array
        let mut member_values: Vec<FieldElement> = Vec::new();
        member_values.extend(keys.clone());
        member_values.extend(values);

        let insert_models: Vec<_> = primitive_members
            .into_iter()
            .zip(member_values.into_iter())
            .map(|((id, name, ty), value)| {
                format!(
                    "INSERT OR REPLACE INTO [{id}] (entity_id, external_{name}) VALUES \
                     ('{entity_id}' {})",
                    CairoType::from_str(&ty).unwrap().format_for_sql(vec![&value]).unwrap()
                )
            })
            .collect();

        // tx commit required
        self.query_queue.extend(insert_models);
        self.execute().await?;

        let query_result = sqlx::query("SELECT created_at FROM entities WHERE id = ?")
            .bind(entity_id.clone())
            .fetch_one(&self.pool)
            .await?;
        let created_at: DateTime<Utc> = query_result.try_get("created_at")?;

        SimpleBroker::publish(Entity {
            id: entity_id.clone(),
            keys: keys_str,
            model_names,
            created_at,
            updated_at: Utc::now(),
        });
        Ok(())
    }

    pub async fn delete_entity(&mut self, model: String, key: FieldElement) -> Result<()> {
        let query = format!("DELETE FROM {model} WHERE id = {key}");
        self.query_queue.push(query);
        Ok(())
    }

    pub async fn entity(&self, model: String, key: FieldElement) -> Result<Vec<FieldElement>> {
        let query = format!("SELECT * FROM {model} WHERE id = {key}");
        let mut conn: PoolConnection<Sqlite> = self.pool.acquire().await?;
        let row: (i32, String, String) = sqlx::query_as(&query).fetch_one(&mut conn).await?;
        Ok(serde_json::from_str(&row.2).unwrap())
    }

    pub async fn entities(&self, model: String) -> Result<Vec<Vec<FieldElement>>> {
        let query = format!("SELECT * FROM {model}");
        let mut conn: PoolConnection<Sqlite> = self.pool.acquire().await?;
        let mut rows =
            sqlx::query_as::<_, (i32, String, String)>(&query).fetch_all(&mut conn).await?;
        Ok(rows.drain(..).map(|row| serde_json::from_str(&row.2).unwrap()).collect())
    }

    pub async fn store_system_call(
        &mut self,
        system: String,
        transaction_hash: FieldElement,
        calldata: &[FieldElement],
    ) -> Result<()> {
        let query = format!(
            "INSERT OR IGNORE INTO system_calls (data, transaction_hash, system_id) VALUES ('{}', \
             '{:#x}', '{}')",
            calldata.iter().map(|c| format!("{:#x}", c)).collect::<Vec<String>>().join(","),
            transaction_hash,
            system
        );
        self.query_queue.push(query);
        Ok(())
    }

    pub async fn store_event(
        &mut self,
        event: &Event,
        event_idx: usize,
        transaction_hash: FieldElement,
    ) -> Result<()> {
        let keys_str = felts_sql_string(&event.keys);
        let data_str = felts_sql_string(&event.data);

        let id = format!("{:#x}:{}", transaction_hash, event_idx);
        let query = format!(
            "INSERT OR IGNORE INTO events (id, keys, data, transaction_hash) VALUES ('{}', '{}', \
             '{}', '{:#x}')",
            id, keys_str, data_str, transaction_hash
        );

        self.query_queue.push(query);
        Ok(())
    }

    fn build_queries_recursive(
        &mut self,
        model: &Ty,
        table_path: Vec<String>,
        model_idx: &mut usize,
    ) {
        self.build_model_query(table_path.clone(), model, *model_idx);

        match model {
            Ty::Struct(s) => {
                for member in s.children.iter() {
                    if let Ty::Primitive(_) = member.ty {
                        continue;
                    }

                    let mut table_path_clone = table_path.clone();
                    table_path_clone.push(member.ty.name());

                    self.build_queries_recursive(
                        &member.ty,
                        table_path_clone,
                        &mut (*model_idx + 1),
                    );
                }
            }
            Ty::Enum(e) => {
                for child in e.children.iter() {
                    let mut table_path_clone = table_path.clone();
                    table_path_clone.push(child.1.name());
                    self.build_model_query(table_path_clone.clone(), &child.1, *model_idx);
                    self.build_queries_recursive(&child.1, table_path_clone, &mut (*model_idx + 1));
                }
            }
            _ => {}
        }
    }

    fn build_model_query(&mut self, table_path: Vec<String>, model: &Ty, model_idx: usize) {
        let table_id = table_path.join("$");

        let mut query = format!(
            "CREATE TABLE IF NOT EXISTS [{table_id}] (entity_id TEXT NOT NULL PRIMARY KEY, "
        );

        match model {
            Ty::Struct(s) => {
                for (member_idx, member) in s.children.iter().enumerate() {
                    if let Ok(cairo_type) = CairoType::from_str(&member.ty.name()) {
                        query.push_str(&format!(
                            "external_{} {}, ",
                            member.name,
                            cairo_type.to_sql_type()
                        ));
                    };

                    self.query_queue.push(format!(
                        "INSERT OR IGNORE INTO model_members (id, model_id, model_idx, \
                         member_idx, name, type, key) VALUES ('{table_id}', '{}', '{model_idx}', \
                         '{member_idx}', '{}', '{}', {})",
                        table_path[0],
                        member.name,
                        member.ty.name(),
                        member.key,
                    ));
                }
            }
            Ty::Enum(_) => {}
            _ => {}
        }

        query.push_str("created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP, ");

        // If this is not the Model's root table, create a reference to the parent.
        if table_path.len() > 1 {
            let parent_table_id = table_path[..table_path.len() - 1].join("$");
            query.push_str(&format!(
                "FOREIGN KEY (entity_id) REFERENCES {parent_table_id} (entity_id), "
            ));
        };

        query.push_str("FOREIGN KEY (entity_id) REFERENCES entities(id));");
        self.query_queue.push(query);
    }

    pub async fn execute(&mut self) -> Result<()> {
        let queries = self.query_queue.clone();
        self.query_queue.clear();

        let mut tx = self.pool.begin().await?;
        for query in queries {
            tx.execute(sqlx::query(&query)).await?;
        }

        tx.commit().await?;

        Ok(())
    }
}

fn model_names_sql_string(entity_result: Option<SqliteRow>, new_model: &str) -> Result<String> {
    let model_names = match entity_result {
        Some(entity) => {
            let existing = entity.try_get::<String, &str>("model_names")?;
            if existing.contains(new_model) {
                existing
            } else {
                format!("{},{}", existing, new_model)
            }
        }
        None => new_model.to_string(),
    };

    Ok(model_names)
}

fn felts_sql_string(felts: &[FieldElement]) -> String {
    felts.iter().map(|k| format!("{:#x}", k)).collect::<Vec<String>>().join("/") + "/"
}
