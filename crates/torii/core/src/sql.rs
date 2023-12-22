use std::convert::TryInto;
use std::str::FromStr;

use anyhow::{anyhow, Result};
use chrono::Utc;
use dojo_types::primitive::Primitive;
use dojo_types::schema::Ty;
use dojo_world::metadata::WorldMetadata;
use sqlx::pool::PoolConnection;
use sqlx::{Pool, Sqlite};
use starknet::core::types::{Event, FieldElement, InvokeTransactionV1};
use starknet_crypto::poseidon_hash_many;

use super::World;
use crate::model::ModelSQLReader;
use crate::query_queue::{Argument, QueryQueue};
use crate::simple_broker::SimpleBroker;
use crate::types::{Entity as EntityUpdated, Event as EventEmitted, Model as ModelRegistered};

pub const FELT_DELIMITER: &str = "/";

#[cfg(test)]
#[path = "sql_test.rs"]
mod test;

#[derive(Debug, Clone)]
pub struct Sql {
    world_address: FieldElement,
    pool: Pool<Sqlite>,
    query_queue: QueryQueue,
}

impl Sql {
    pub async fn new(pool: Pool<Sqlite>, world_address: FieldElement) -> Result<Self> {
        let mut query_queue = QueryQueue::new(pool.clone());

        query_queue.enqueue(
            "INSERT OR IGNORE INTO indexers (id, head) VALUES (?, ?)",
            vec![Argument::FieldElement(world_address), Argument::Int(0)],
        );
        query_queue.enqueue(
            "INSERT OR IGNORE INTO worlds (id, world_address) VALUES (?, ?)",
            vec![Argument::FieldElement(world_address), Argument::FieldElement(world_address)],
        );

        query_queue.execute_all().await?;

        Ok(Self { pool, world_address, query_queue })
    }

    pub async fn head(&self) -> Result<u64> {
        let mut conn: PoolConnection<Sqlite> = self.pool.acquire().await?;
        let indexer_query = sqlx::query_as::<_, (i64,)>("SELECT head FROM indexers WHERE id = ?")
            .bind(format!("{:#x}", self.world_address));

        let indexer: (i64,) = indexer_query.fetch_one(&mut *conn).await?;
        Ok(indexer.0.try_into().expect("doesn't fit in u64"))
    }

    pub fn set_head(&mut self, head: u64) {
        let head = Argument::Int(head.try_into().expect("doesn't fit in u64"));
        let id = Argument::String(format!("{:#x}", self.world_address));

        self.query_queue.enqueue("UPDATE indexers SET head = ? WHERE id = ?", vec![head, id]);
    }

    pub async fn world(&self) -> Result<World> {
        let mut conn: PoolConnection<Sqlite> = self.pool.acquire().await?;
        let meta: World = sqlx::query_as("SELECT * FROM worlds WHERE id = ?")
            .bind(format!("{:#x}", self.world_address))
            .fetch_one(&mut *conn)
            .await?;

        Ok(meta)
    }

    pub async fn register_model(
        &mut self,
        model: Ty,
        layout: Vec<FieldElement>,
        class_hash: FieldElement,
        packed_size: u32,
        unpacked_size: u32,
    ) -> Result<()> {
        let layout_blob = layout
            .iter()
            .map(|x| <FieldElement as TryInto<u8>>::try_into(*x).unwrap())
            .collect::<Vec<u8>>();

        let insert_models = "INSERT INTO models (id, name, class_hash, layout, packed_size, \
                             unpacked_size) VALUES (?, ?, ?, ?, ?, ?) ON CONFLICT(id) DO UPDATE \
                             SET class_hash=EXCLUDED.class_hash, layout=EXCLUDED.layout, \
                             packed_size=EXCLUDED.packed_size, \
                             unpacked_size=EXCLUDED.unpacked_size RETURNING *";
        let model_registered: ModelRegistered = sqlx::query_as(insert_models)
            .bind(model.name())
            .bind(model.name())
            .bind(format!("{class_hash:#x}"))
            .bind(hex::encode(&layout_blob))
            .bind(packed_size)
            .bind(unpacked_size)
            .fetch_one(&self.pool)
            .await?;

        let mut model_idx = 0_i64;
        self.build_register_queries_recursive(&model, vec![model.name()], &mut model_idx);
        self.query_queue.execute_all().await?;

        SimpleBroker::publish(model_registered);

        Ok(())
    }

    pub async fn set_entity(&mut self, entity: Ty, event_id: &str) -> Result<()> {
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
        self.query_queue.enqueue(
            "INSERT INTO entity_model (entity_id, model_id) VALUES (?, ?) ON CONFLICT(entity_id, \
             model_id) DO NOTHING",
            vec![Argument::String(entity_id.clone()), Argument::String(entity.name())],
        );

        let keys_str = felts_sql_string(&keys);
        let insert_entities = "INSERT INTO entities (id, keys, event_id) VALUES (?, ?, ?) ON \
                               CONFLICT(id) DO UPDATE SET updated_at=CURRENT_TIMESTAMP, \
                               event_id=EXCLUDED.event_id RETURNING *";
        let entity_updated: EntityUpdated = sqlx::query_as(insert_entities)
            .bind(&entity_id)
            .bind(&keys_str)
            .bind(event_id)
            .fetch_one(&self.pool)
            .await?;

        let path = vec![entity.name()];
        self.build_set_entity_queries_recursive(path, event_id, &entity_id, &entity);
        self.query_queue.execute_all().await?;

        SimpleBroker::publish(entity_updated);

        Ok(())
    }

    pub fn delete_entity(&mut self, model: String, key: FieldElement) {
        let model = Argument::String(model);
        let id = Argument::FieldElement(key);

        self.query_queue.enqueue("DELETE FROM ? WHERE id = ?", vec![model, id]);
    }

    pub fn set_metadata(&mut self, resource: &FieldElement, uri: &str) {
        let resource = Argument::FieldElement(*resource);
        let uri = Argument::String(uri.to_string());

        self.query_queue.enqueue(
            "INSERT INTO metadata (id, uri) VALUES (?, ?) ON CONFLICT(id) DO UPDATE SET \
             id=excluded.id, updated_at=CURRENT_TIMESTAMP",
            vec![resource, uri],
        );
    }

    pub async fn update_metadata(
        &mut self,
        resource: &FieldElement,
        uri: &str,
        metadata: &WorldMetadata,
        icon_img: &Option<String>,
        cover_img: &Option<String>,
    ) -> Result<()> {
        let json = serde_json::to_string(metadata).unwrap(); // safe unwrap

        let mut columns = vec!["id", "uri", "json"];
        let mut update =
            vec!["id=excluded.id", "json=excluded.json", "updated_at=CURRENT_TIMESTAMP"];
        let mut arguments = vec![
            Argument::FieldElement(*resource),
            Argument::String(uri.to_string()),
            Argument::String(json),
        ];

        if let Some(icon) = icon_img {
            columns.push("icon_img");
            arguments.push(Argument::String(icon.clone()));
            update.push("icon_img=excluded.icon_img");
        }

        if let Some(cover) = cover_img {
            columns.push("cover_img");
            arguments.push(Argument::String(cover.clone()));
            update.push("cover_img=excluded.cover_img");
        }

        let placeholders: Vec<&str> = arguments.iter().map(|_| "?").collect();
        let statement = format!(
            "INSERT INTO metadata ({}) VALUES ({}) ON CONFLICT(id) DO UPDATE SET {}",
            columns.join(","),
            placeholders.join(","),
            update.join(",")
        );

        self.query_queue.enqueue(statement, arguments);
        self.query_queue.execute_all().await?;

        Ok(())
    }

    pub async fn model(&self, model: &str) -> Result<ModelSQLReader> {
        let reader = ModelSQLReader::new(model, self.pool.clone()).await?;
        Ok(reader)
    }

    pub async fn entity(&self, model: String, key: FieldElement) -> Result<Vec<FieldElement>> {
        let query = sqlx::query_as::<_, (i32, String, String)>("SELECT * FROM ? WHERE id = ?")
            .bind(model)
            .bind(format!("{:#x}", key));

        let mut conn: PoolConnection<Sqlite> = self.pool.acquire().await?;
        let row: (i32, String, String) = query.fetch_one(&mut *conn).await?;
        Ok(serde_json::from_str(&row.2).unwrap())
    }

    pub async fn entities(&self, model: String) -> Result<Vec<Vec<FieldElement>>> {
        let query = sqlx::query_as::<_, (i32, String, String)>("SELECT * FROM ?").bind(model);
        let mut conn: PoolConnection<Sqlite> = self.pool.acquire().await?;
        let mut rows = query.fetch_all(&mut *conn).await?;
        Ok(rows.drain(..).map(|row| serde_json::from_str(&row.2).unwrap()).collect())
    }

    pub fn store_transaction(&mut self, transaction: &InvokeTransactionV1, transaction_id: &str) {
        let id = Argument::String(transaction_id.to_string());
        let transaction_hash = Argument::FieldElement(transaction.transaction_hash);
        let sender_address = Argument::FieldElement(transaction.sender_address);
        let calldata = Argument::String(felts_sql_string(&transaction.calldata));
        let max_fee = Argument::FieldElement(transaction.max_fee);
        let signature = Argument::String(felts_sql_string(&transaction.signature));
        let nonce = Argument::FieldElement(transaction.nonce);

        self.query_queue.enqueue(
            "INSERT OR IGNORE INTO transactions (id, transaction_hash, sender_address, calldata, \
             max_fee, signature, nonce) VALUES (?, ?, ?, ?, ?, ?, ?)",
            vec![id, transaction_hash, sender_address, calldata, max_fee, signature, nonce],
        );
    }

    pub fn store_event(&mut self, event_id: &str, event: &Event, transaction_hash: FieldElement) {
        let id = Argument::String(event_id.to_string());
        let keys = Argument::String(felts_sql_string(&event.keys));
        let data = Argument::String(felts_sql_string(&event.data));
        let hash = Argument::FieldElement(transaction_hash);

        self.query_queue.enqueue(
            "INSERT OR IGNORE INTO events (id, keys, data, transaction_hash) VALUES (?, ?, ?, ?)",
            vec![id, keys, data, hash],
        );

        SimpleBroker::publish(EventEmitted {
            id: event_id.to_string(),
            keys: felts_sql_string(&event.keys),
            data: felts_sql_string(&event.data),
            transaction_hash: format!("{:#x}", transaction_hash),
            created_at: Utc::now(),
        });
    }

    fn build_register_queries_recursive(
        &mut self,
        model: &Ty,
        path: Vec<String>,
        model_idx: &mut i64,
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

    fn build_set_entity_queries_recursive(
        &mut self,
        path: Vec<String>,
        event_id: &str,
        entity_id: &str,
        entity: &Ty,
    ) {
        match entity {
            Ty::Struct(s) => {
                let table_id = path.join("$");
                let mut columns = vec!["entity_id".to_string(), "event_id".to_string()];
                let mut arguments = vec![
                    Argument::String(entity_id.to_string()),
                    Argument::String(event_id.to_string()),
                ];

                for member in s.children.iter() {
                    match &member.ty {
                        Ty::Primitive(ty) => {
                            columns.push(format!("external_{}", &member.name));
                            arguments.push(Argument::String(ty.to_sql_value().unwrap()));
                        }
                        Ty::Enum(e) => {
                            columns.push(format!("external_{}", &member.name));
                            arguments.push(Argument::String(e.to_sql_value().unwrap()));
                        }
                        _ => {}
                    }
                }

                let placeholders: Vec<&str> = arguments.iter().map(|_| "?").collect();
                let statement = format!(
                    "INSERT OR REPLACE INTO [{table_id}] ({}) VALUES ({})",
                    columns.join(","),
                    placeholders.join(",")
                );
                self.query_queue.enqueue(statement, arguments);

                for member in s.children.iter() {
                    if let Ty::Struct(_) = &member.ty {
                        let mut path_clone = path.clone();
                        path_clone.push(member.ty.name());

                        self.build_set_entity_queries_recursive(
                            path_clone, event_id, entity_id, &member.ty,
                        );
                    }
                }
            }
            Ty::Enum(e) => {
                for child in e.options.iter() {
                    let mut path_clone = path.clone();
                    path_clone.push(child.ty.name());
                    self.build_set_entity_queries_recursive(
                        path_clone, event_id, entity_id, &child.ty,
                    );
                }
            }
            _ => {}
        }
    }

    fn build_model_query(&mut self, path: Vec<String>, model: &Ty, model_idx: i64) {
        let table_id = path.join("$");
        let mut indices = Vec::new();

        let mut create_table_query = format!(
            "CREATE TABLE IF NOT EXISTS [{table_id}] (entity_id TEXT NOT NULL PRIMARY KEY, \
             event_id, "
        );

        if let Ty::Struct(s) = model {
            for (member_idx, member) in s.children.iter().enumerate() {
                let name = member.name.clone();
                let mut options = None; // TEMP: doesnt support complex enums yet

                if let Ok(cairo_type) = Primitive::from_str(&member.ty.name()) {
                    create_table_query
                        .push_str(&format!("external_{name} {}, ", cairo_type.to_sql_type()));
                    indices.push(format!(
                        "CREATE INDEX IF NOT EXISTS idx_{table_id}_{name} ON [{table_id}] \
                         (external_{name});"
                    ));
                } else if let Ty::Enum(e) = &member.ty {
                    let all_options = e
                        .options
                        .iter()
                        .map(|c| format!("'{}'", c.name))
                        .collect::<Vec<_>>()
                        .join(", ");

                    create_table_query.push_str(&format!(
                        "external_{name} TEXT CHECK(external_{name} IN ({all_options})) NOT NULL, ",
                    ));

                    indices.push(format!(
                        "CREATE INDEX IF NOT EXISTS idx_{table_id}_{name} ON [{table_id}] \
                         (external_{name});"
                    ));

                    options = Some(Argument::String(
                        e.options
                            .iter()
                            .map(|c| c.name.clone())
                            .collect::<Vec<_>>()
                            .join(",")
                            .to_string(),
                    ));
                }

                let statement = "INSERT OR IGNORE INTO model_members (id, model_id, model_idx, \
                                 member_idx, name, type, type_enum, enum_options, key) VALUES (?, \
                                 ?, ?, ?, ?, ?, ?, ?, ?)";
                let arguments = vec![
                    Argument::String(table_id.clone()),
                    Argument::String(path[0].clone()),
                    Argument::Int(model_idx),
                    Argument::Int(member_idx as i64),
                    Argument::String(name),
                    Argument::String(member.ty.name()),
                    Argument::String(member.ty.as_ref().into()),
                    options.unwrap_or(Argument::Null),
                    Argument::Bool(member.key),
                ];

                self.query_queue.enqueue(statement, arguments);
            }
        }

        create_table_query.push_str("created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP, ");

        // If this is not the Model's root table, create a reference to the parent.
        if path.len() > 1 {
            let parent_table_id = path[..path.len() - 1].join("$");
            create_table_query.push_str(&format!(
                "FOREIGN KEY (entity_id) REFERENCES {parent_table_id} (entity_id), "
            ));
        };

        create_table_query.push_str("FOREIGN KEY (entity_id) REFERENCES entities(id));");
        self.query_queue.enqueue(create_table_query, vec![]);

        indices.iter().for_each(|s| {
            self.query_queue.enqueue(s, vec![]);
        });
    }

    pub async fn execute(&mut self) -> Result<()> {
        self.query_queue.execute_all().await?;

        Ok(())
    }
}

fn felts_sql_string(felts: &[FieldElement]) -> String {
    felts.iter().map(|k| format!("{:#x}", k)).collect::<Vec<String>>().join(FELT_DELIMITER)
        + FELT_DELIMITER
}
