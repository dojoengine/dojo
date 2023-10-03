use std::str::FromStr;

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use dojo_types::primitive::Primitive;
use dojo_types::schema::Ty;
use dojo_world::manifest::System;
use sqlx::pool::PoolConnection;
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

    pub fn set_head(&mut self, head: u64) {
        self.query_queue.push(format!(
            "UPDATE indexers SET head = {head} WHERE id = '{:#x}'",
            self.world_address
        ));
    }

    pub async fn world(&self) -> Result<World> {
        let mut conn: PoolConnection<Sqlite> = self.pool.acquire().await?;
        let meta: World =
            sqlx::query_as(&format!("SELECT * FROM worlds WHERE id = '{:#x}'", self.world_address))
                .fetch_one(&mut conn)
                .await?;

        Ok(meta)
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
        self.build_register_queries_recursive(&model, vec![model.name()], &mut model_idx);

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

    pub async fn set_entity(&mut self, entity: Ty) -> Result<()> {
        let keys = if let Ty::Struct(s) = &entity {
            let mut keys = Vec::new();
            for m in s.keys() {
                keys.extend(m.serialize()?);
            }
            keys
        } else {
            return Err(anyhow!("Entity is not a struct"));
        };

        let entity_id = format!("{:#x}", poseidon_hash_many(&keys));
        let existing: Option<(String,)> =
            sqlx::query_as("SELECT model_names FROM entities WHERE id = ?")
                .bind(&entity_id)
                .fetch_optional(&self.pool)
                .await?;

        let model_names = if let Some((model_names,)) = existing {
            format!("{},{}", model_names, entity.name())
        } else {
            entity.name()
        };

        let keys_str = felts_sql_string(&keys);
        self.query_queue.push(format!(
            "INSERT INTO entities (id, keys, model_names) VALUES ('{}', '{}', '{}') ON \
             CONFLICT(id) DO UPDATE SET model_names=excluded.model_names, \
             updated_at=CURRENT_TIMESTAMP",
            entity_id, keys_str, model_names
        ));

        let path = vec![entity.name()];
        self.build_set_entity_queries_recursive(path, &entity_id, &entity);

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

    pub fn delete_entity(&mut self, model: String, key: FieldElement) {
        let query = format!("DELETE FROM {model} WHERE id = {key}");
        self.query_queue.push(query);
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

    pub fn store_system_call(
        &mut self,
        system: String,
        transaction_hash: FieldElement,
        calldata: &[FieldElement],
    ) {
        let query = format!(
            "INSERT OR IGNORE INTO system_calls (data, transaction_hash, system_id) VALUES ('{}', \
             '{:#x}', '{}')",
            calldata.iter().map(|c| format!("{:#x}", c)).collect::<Vec<String>>().join(","),
            transaction_hash,
            system
        );
        self.query_queue.push(query);
    }

    pub fn store_event(&mut self, event: &Event, event_idx: usize, transaction_hash: FieldElement) {
        let keys_str = felts_sql_string(&event.keys);
        let data_str = felts_sql_string(&event.data);

        let id = format!("{:#x}:{}", transaction_hash, event_idx);
        let query = format!(
            "INSERT OR IGNORE INTO events (id, keys, data, transaction_hash) VALUES ('{}', '{}', \
             '{}', '{:#x}')",
            id, keys_str, data_str, transaction_hash
        );

        self.query_queue.push(query);
    }

    fn build_register_queries_recursive(
        &mut self,
        model: &Ty,
        path: Vec<String>,
        model_idx: &mut usize,
    ) {
        if let Ty::Enum(_) = model {
            // Complex enum values not supported yet.
            return;
        }

        self.build_model_query(path.clone(), model, *model_idx);

        if let Ty::Struct(s) = model {
            for member in s.children.iter() {
                if let Ty::Primitive(_) = member.ty {
                    continue;
                }

                let mut path_clone = path.clone();
                path_clone.push(member.ty.name());

                self.build_register_queries_recursive(
                    &member.ty,
                    path_clone,
                    &mut (*model_idx + 1),
                );
            }
        }
    }

    fn build_set_entity_queries_recursive(&mut self, path: Vec<String>, id: &str, entity: &Ty) {
        match entity {
            Ty::Struct(s) => {
                let table_id = path.join("$");
                let mut columns = vec!["entity_id".to_string()];
                let mut values = vec![format!("'{id}'")];

                for member in s.children.iter() {
                    match &member.ty {
                        Ty::Primitive(ty) => {
                            columns.push(format!("external_{}", &member.name));
                            values.push(ty.to_sql_value().unwrap());
                        }
                        Ty::Enum(e) => {
                            columns.push(format!("external_{}", &member.name));
                            values.push(e.to_sql_value().unwrap());
                        }
                        _ => {}
                    }
                }

                self.query_queue.push(format!(
                    "INSERT OR REPLACE INTO [{table_id}] ({}) VALUES ({})",
                    columns.join(", "),
                    values.join(", ")
                ));

                for member in s.children.iter() {
                    if let Ty::Struct(_) = &member.ty {
                        let mut path_clone = path.clone();
                        path_clone.push(member.ty.name());

                        self.build_set_entity_queries_recursive(path_clone, id, &member.ty);
                    }
                }
            }
            Ty::Enum(e) => {
                for child in e.options.iter() {
                    let mut path_clone = path.clone();
                    path_clone.push(child.1.name());
                    // self.build_entity_query(path_clone.clone(), id, &child.1);
                    self.build_set_entity_queries_recursive(path_clone, id, &child.1);
                }
            }
            _ => {}
        }
    }

    fn build_model_query(&mut self, path: Vec<String>, model: &Ty, model_idx: usize) {
        let table_id = path.join("$");

        let mut query = format!(
            "CREATE TABLE IF NOT EXISTS [{table_id}] (entity_id TEXT NOT NULL PRIMARY KEY, "
        );

        if let Ty::Struct(s) = model {
            for (member_idx, member) in s.children.iter().enumerate() {
                let name = member.name.clone();
                if let Ok(cairo_type) = Primitive::from_str(&member.ty.name()) {
                    query.push_str(&format!("external_{name} {}, ", cairo_type.to_sql_type()));
                } else if let Ty::Enum(e) = &member.ty {
                    let options = e
                        .options
                        .iter()
                        .map(|c| format!("'{}'", c.0))
                        .collect::<Vec<_>>()
                        .join(", ");
                    query.push_str(&format!(
                        "external_{name} TEXT CHECK(external_{name} IN ({options})) NOT NULL, ",
                    ));
                }

                self.query_queue.push(format!(
                    "INSERT OR IGNORE INTO model_members (id, model_id, model_idx, member_idx, \
                     name, type, key) VALUES ('{table_id}', '{}', '{model_idx}', '{member_idx}', \
                     '{name}', '{}', {})",
                    path[0],
                    member.ty.name(),
                    member.key,
                ));
            }
        }

        query.push_str("created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP, ");

        // If this is not the Model's root table, create a reference to the parent.
        if path.len() > 1 {
            let parent_table_id = path[..path.len() - 1].join("$");
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

fn felts_sql_string(felts: &[FieldElement]) -> String {
    felts.iter().map(|k| format!("{:#x}", k)).collect::<Vec<String>>().join("/") + "/"
}
