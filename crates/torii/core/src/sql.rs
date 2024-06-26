use std::convert::TryInto;
use std::str::FromStr;

use anyhow::{anyhow, Result};
use chrono::Utc;
use dojo_types::primitive::Primitive;
use dojo_types::schema::{EnumOption, Member, Ty};
use dojo_world::contracts::abi::model::Layout;
use dojo_world::metadata::WorldMetadata;
use sqlx::pool::PoolConnection;
use sqlx::{Pool, Sqlite};
use starknet::core::types::{Event, FieldElement, InvokeTransaction, Transaction};
use starknet::core::utils::get_selector_from_name;
use starknet_crypto::poseidon_hash_many;

use super::World;
use crate::model::ModelSQLReader;
use crate::query_queue::{Argument, QueryQueue};
use crate::simple_broker::SimpleBroker;
use crate::types::{
    Entity as EntityUpdated, Event as EventEmitted, EventMessage as EventMessageUpdated,
    Model as ModelRegistered,
};
use crate::utils::{must_utc_datetime_from_timestamp, utc_dt_string_from_timestamp};

pub const FELT_DELIMITER: &str = "/";

#[cfg(test)]
#[path = "sql_test.rs"]
mod test;

#[derive(Debug, Clone)]
pub struct Sql {
    world_address: FieldElement,
    pub pool: Pool<Sqlite>,
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

    pub async fn head(&self) -> Result<(u64, Option<FieldElement>)> {
        let mut conn: PoolConnection<Sqlite> = self.pool.acquire().await?;
        let indexer_query = sqlx::query_as::<_, (i64, Option<String>)>(
            "SELECT head, pending_block_tx FROM indexers WHERE id = ?",
        )
        .bind(format!("{:#x}", self.world_address));

        let indexer: (i64, Option<String>) = indexer_query.fetch_one(&mut *conn).await?;
        Ok((
            indexer.0.try_into().expect("doesn't fit in u64"),
            indexer.1.map(|f| FieldElement::from_str(&f)).transpose()?,
        ))
    }

    pub fn set_head(&mut self, head: u64, pending_block_tx: Option<FieldElement>) {
        let head = Argument::Int(head.try_into().expect("doesn't fit in u64"));
        let id = Argument::FieldElement(self.world_address);
        let pending_block_tx = if let Some(f) = pending_block_tx {
            Argument::String(format!("{:#x}", f))
        } else {
            Argument::Null
        };

        self.query_queue.enqueue(
            "UPDATE indexers SET head = ?, pending_block_tx = ? WHERE id = ?",
            vec![head, pending_block_tx, id],
        );
    }

    pub async fn world(&self) -> Result<World> {
        let mut conn: PoolConnection<Sqlite> = self.pool.acquire().await?;
        let meta: World = sqlx::query_as("SELECT * FROM worlds WHERE id = ?")
            .bind(format!("{:#x}", self.world_address))
            .fetch_one(&mut *conn)
            .await?;

        Ok(meta)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn register_model(
        &mut self,
        model: Ty,
        layout: Layout,
        class_hash: FieldElement,
        contract_address: FieldElement,
        packed_size: u32,
        unpacked_size: u32,
        block_timestamp: u64,
    ) -> Result<()> {
        let insert_models =
            "INSERT INTO models (id, name, class_hash, contract_address, layout, packed_size, \
             unpacked_size, executed_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT(id) DO \
             UPDATE SET contract_address=EXCLUDED.contract_address, \
             class_hash=EXCLUDED.class_hash, layout=EXCLUDED.layout, \
             packed_size=EXCLUDED.packed_size, unpacked_size=EXCLUDED.unpacked_size, \
             executed_at=EXCLUDED.executed_at RETURNING *";
        let model_registered: ModelRegistered = sqlx::query_as(insert_models)
            // this is temporary until the model hash is precomputed
            .bind(&format!("{:#x}", &get_selector_from_name(&model.name())?))
            .bind(model.name())
            .bind(format!("{class_hash:#x}"))
            .bind(format!("{contract_address:#x}"))
            .bind(serde_json::to_string(&layout)?)
            .bind(packed_size)
            .bind(unpacked_size)
            .bind(utc_dt_string_from_timestamp(block_timestamp))
            .fetch_one(&self.pool)
            .await?;

        let mut model_idx = 0_i64;
        self.build_register_queries_recursive(
            &model,
            vec![model.name()],
            &mut model_idx,
            block_timestamp,
            &mut 0,
            &mut 0,
        );
        self.query_queue.execute_all().await?;

        SimpleBroker::publish(model_registered);

        Ok(())
    }

    pub async fn set_entity(
        &mut self,
        entity: Ty,
        event_id: &str,
        block_timestamp: u64,
    ) -> Result<()> {
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
            vec![
                Argument::String(entity_id.clone()),
                Argument::String(format!("{:#x}", get_selector_from_name(&entity.name())?)),
            ],
        );

        let keys_str = felts_sql_string(&keys);
        let insert_entities = "INSERT INTO entities (id, keys, event_id, executed_at) VALUES (?, \
                               ?, ?, ?) ON CONFLICT(id) DO UPDATE SET \
                               executed_at=EXCLUDED.executed_at, event_id=EXCLUDED.event_id \
                               RETURNING *";
        let mut entity_updated: EntityUpdated = sqlx::query_as(insert_entities)
            .bind(&entity_id)
            .bind(&keys_str)
            .bind(event_id)
            .bind(utc_dt_string_from_timestamp(block_timestamp))
            .fetch_one(&self.pool)
            .await?;

        entity_updated.updated_model = Some(entity.clone());

        let path = vec![entity.name()];
        self.build_set_entity_queries_recursive(
            path,
            event_id,
            (&entity_id, false),
            &entity,
            block_timestamp,
            &vec![],
        );
        self.query_queue.execute_all().await?;

        SimpleBroker::publish(entity_updated);

        Ok(())
    }

    pub async fn set_event_message(
        &mut self,
        entity: Ty,
        event_id: &str,
        block_timestamp: u64,
    ) -> Result<()> {
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
            "INSERT INTO event_model (entity_id, model_id) VALUES (?, ?) ON CONFLICT(entity_id, \
             model_id) DO NOTHING",
            vec![
                Argument::String(entity_id.clone()),
                Argument::String(format!("{:#x}", get_selector_from_name(&entity.name())?)),
            ],
        );

        let keys_str = felts_sql_string(&keys);
        let insert_entities = "INSERT INTO event_messages (id, keys, event_id, executed_at) \
                               VALUES (?, ?, ?, ?) ON CONFLICT(id) DO UPDATE SET \
                               updated_at=CURRENT_TIMESTAMP, event_id=EXCLUDED.event_id RETURNING \
                               *";
        let mut event_message_updated: EventMessageUpdated = sqlx::query_as(insert_entities)
            .bind(&entity_id)
            .bind(&keys_str)
            .bind(event_id)
            .bind(utc_dt_string_from_timestamp(block_timestamp))
            .fetch_one(&self.pool)
            .await?;

        event_message_updated.updated_model = Some(entity.clone());

        let path = vec![entity.name()];
        self.build_set_entity_queries_recursive(
            path,
            event_id,
            (&entity_id, true),
            &entity,
            block_timestamp,
            &vec![],
        );
        self.query_queue.execute_all().await?;

        SimpleBroker::publish(event_message_updated);

        Ok(())
    }

    pub async fn delete_entity(&mut self, keys: Vec<FieldElement>, entity: Ty) -> Result<()> {
        let entity_id = format!("{:#x}", poseidon_hash_many(&keys));
        let path = vec![entity.name()];
        // delete entity models data
        self.build_delete_entity_queries_recursive(path, &entity_id, &entity);
        self.query_queue.execute_all().await?;

        // delete entity
        let entity_deleted =
            sqlx::query_as::<_, EntityUpdated>("DELETE FROM entities WHERE id = ? RETURNING *")
                .bind(entity_id)
                .fetch_one(&self.pool)
                .await?;

        SimpleBroker::publish(entity_deleted);
        Ok(())
    }

    pub fn set_metadata(&mut self, resource: &FieldElement, uri: &str, block_timestamp: u64) {
        let resource = Argument::FieldElement(*resource);
        let uri = Argument::String(uri.to_string());
        let executed_at = Argument::String(utc_dt_string_from_timestamp(block_timestamp));

        self.query_queue.enqueue(
            "INSERT INTO metadata (id, uri, executed_at) VALUES (?, ?, ?) ON CONFLICT(id) DO \
             UPDATE SET id=excluded.id, executed_at=excluded.executed_at, \
             updated_at=CURRENT_TIMESTAMP",
            vec![resource, uri, executed_at],
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

        let mut update = vec!["uri=?", "json=?", "updated_at=CURRENT_TIMESTAMP"];
        let mut arguments = vec![Argument::String(uri.to_string()), Argument::String(json)];

        if let Some(icon) = icon_img {
            update.push("icon_img=?");
            arguments.push(Argument::String(icon.clone()));
        }

        if let Some(cover) = cover_img {
            update.push("cover_img=?");
            arguments.push(Argument::String(cover.clone()));
        }

        let statement = format!("UPDATE metadata SET {} WHERE id = ?", update.join(","));
        arguments.push(Argument::FieldElement(*resource));

        self.query_queue.enqueue(statement, arguments);
        self.query_queue.execute_all().await?;

        Ok(())
    }

    pub async fn model(&self, model: &str) -> Result<ModelSQLReader> {
        match ModelSQLReader::new(model, self.pool.clone()).await {
            Ok(reader) => Ok(reader),
            Err(e) => Err(anyhow::anyhow!("Failed to get model from db for selector {model}: {e}")),
        }
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

    pub fn store_transaction(
        &mut self,
        transaction: &Transaction,
        transaction_id: &str,
        block_timestamp: u64,
    ) {
        let id = Argument::String(transaction_id.to_string());

        let transaction_type = match transaction {
            Transaction::Invoke(_) => "INVOKE",
            Transaction::L1Handler(_) => "L1_HANDLER",
            _ => return,
        };

        let (transaction_hash, sender_address, calldata, max_fee, signature, nonce) =
            match transaction {
                Transaction::Invoke(InvokeTransaction::V1(invoke_v1_transaction)) => (
                    Argument::FieldElement(invoke_v1_transaction.transaction_hash),
                    Argument::FieldElement(invoke_v1_transaction.sender_address),
                    Argument::String(felts_sql_string(&invoke_v1_transaction.calldata)),
                    Argument::FieldElement(invoke_v1_transaction.max_fee),
                    Argument::String(felts_sql_string(&invoke_v1_transaction.signature)),
                    Argument::FieldElement(invoke_v1_transaction.nonce),
                ),
                Transaction::L1Handler(l1_handler_transaction) => (
                    Argument::FieldElement(l1_handler_transaction.transaction_hash),
                    Argument::FieldElement(l1_handler_transaction.contract_address),
                    Argument::String(felts_sql_string(&l1_handler_transaction.calldata)),
                    Argument::FieldElement(FieldElement::ZERO), // has no max_fee
                    Argument::String("".to_string()),           // has no signature
                    Argument::FieldElement((l1_handler_transaction.nonce).into()),
                ),
                _ => return,
            };

        self.query_queue.enqueue(
            "INSERT OR IGNORE INTO transactions (id, transaction_hash, sender_address, calldata, \
             max_fee, signature, nonce, transaction_type, executed_at) VALUES (?, ?, ?, ?, ?, ?, \
             ?, ?, ?)",
            vec![
                id,
                transaction_hash,
                sender_address,
                calldata,
                max_fee,
                signature,
                nonce,
                Argument::String(transaction_type.to_string()),
                Argument::String(utc_dt_string_from_timestamp(block_timestamp)),
            ],
        );
    }

    pub fn store_event(
        &mut self,
        event_id: &str,
        event: &Event,
        transaction_hash: FieldElement,
        block_timestamp: u64,
    ) {
        let id = Argument::String(event_id.to_string());
        let keys = Argument::String(felts_sql_string(&event.keys));
        let data = Argument::String(felts_sql_string(&event.data));
        let hash = Argument::FieldElement(transaction_hash);
        let executed_at = Argument::String(utc_dt_string_from_timestamp(block_timestamp));

        self.query_queue.enqueue(
            "INSERT OR IGNORE INTO events (id, keys, data, transaction_hash, executed_at) VALUES \
             (?, ?, ?, ?, ?)",
            vec![id, keys, data, hash, executed_at],
        );

        SimpleBroker::publish(EventEmitted {
            id: event_id.to_string(),
            keys: felts_sql_string(&event.keys),
            data: felts_sql_string(&event.data),
            transaction_hash: format!("{:#x}", transaction_hash),
            created_at: Utc::now(),
            executed_at: must_utc_datetime_from_timestamp(block_timestamp),
        });
    }

    fn build_register_queries_recursive(
        &mut self,
        model: &Ty,
        path: Vec<String>,
        model_idx: &mut i64,
        block_timestamp: u64,
        array_idx: &mut usize,
        parent_array_idx: &mut usize,
    ) {
        if let Ty::Enum(e) = model {
            if e.options.iter().all(|o| if let Ty::Tuple(t) = &o.ty { t.is_empty() } else { false })
            {
                return;
            }
        }

        self.build_model_query(
            path.clone(),
            model,
            *model_idx,
            block_timestamp,
            *array_idx,
            *parent_array_idx,
        );

        let mut build_member = |pathname: &str, member: &Ty| {
            if let Ty::Primitive(_) = member {
                return;
            } else if let Ty::ByteArray(_) = member {
                return;
            }

            let mut path_clone = path.clone();
            path_clone.push(pathname.to_string());

            self.build_register_queries_recursive(
                member,
                path_clone,
                &mut (*model_idx + 1),
                block_timestamp,
                &mut (*array_idx + if let Ty::Array(_) = member { 1 } else { 0 }),
                &mut (*parent_array_idx + if let Ty::Array(_) = model { 1 } else { 0 }),
            );
        };

        if let Ty::Struct(s) = model {
            for member in s.children.iter() {
                build_member(&member.name, &member.ty);
            }
        } else if let Ty::Tuple(t) = model {
            for (idx, member) in t.iter().enumerate() {
                build_member(format!("_{}", idx).as_str(), member);
            }
        } else if let Ty::Array(array) = model {
            let ty = &array[0];
            build_member("data", ty);
        } else if let Ty::Enum(e) = model {
            for child in e.options.iter() {
                // Skip enum options that have no type / member
                if let Ty::Tuple(t) = &child.ty {
                    if t.is_empty() {
                        continue;
                    }
                }

                build_member(&child.name, &child.ty);
            }
        }
    }

    fn build_set_entity_queries_recursive(
        &mut self,
        path: Vec<String>,
        event_id: &str,
        // The id of the entity and if the entity is an event message
        entity_id: (&str, bool),
        entity: &Ty,
        block_timestamp: u64,
        indexes: &Vec<i64>,
    ) {
        let (entity_id, is_event_message) = entity_id;

        let update_members =
            |members: &[Member], query_queue: &mut QueryQueue, indexes: &Vec<i64>| {
                let table_id = path.join("$");
                let mut columns = vec![
                    "id".to_string(),
                    "event_id".to_string(),
                    "executed_at".to_string(),
                    "updated_at".to_string(),
                    if is_event_message {
                        "event_message_id".to_string()
                    } else {
                        "entity_id".to_string()
                    },
                ];

                let mut arguments = vec![
                    Argument::String(if is_event_message {
                        "event:".to_string() + entity_id
                    } else {
                        entity_id.to_string()
                    }),
                    Argument::String(event_id.to_string()),
                    Argument::String(utc_dt_string_from_timestamp(block_timestamp)),
                    Argument::String(chrono::Utc::now().to_rfc3339()),
                    Argument::String(entity_id.to_string()),
                ];

                if !indexes.is_empty() {
                    columns.push("full_array_id".to_string());
                    arguments.push(Argument::String(
                        std::iter::once(entity_id.to_string())
                            .chain(indexes.iter().map(|i| i.to_string()))
                            .collect::<Vec<String>>()
                            .join(FELT_DELIMITER),
                    ));
                }

                for (column_idx, idx) in indexes.iter().enumerate() {
                    columns.push(format!("idx_{}", column_idx));
                    arguments.push(Argument::Int(*idx));
                }

                for member in members.iter() {
                    match &member.ty {
                        Ty::Primitive(ty) => {
                            columns.push(format!("external_{}", &member.name));
                            arguments.push(Argument::String(ty.to_sql_value().unwrap()));
                        }
                        Ty::Enum(e) => {
                            columns.push(format!("external_{}", &member.name));
                            arguments.push(Argument::String(e.to_sql_value().unwrap()));
                        }
                        Ty::ByteArray(b) => {
                            columns.push(format!("external_{}", &member.name));
                            arguments.push(Argument::String(b.clone()));
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

                query_queue.enqueue(statement, arguments);
            };

        match entity {
            Ty::Struct(s) => {
                update_members(&s.children, &mut self.query_queue, indexes);

                for member in s.children.iter() {
                    let mut path_clone = path.clone();
                    path_clone.push(member.name.clone());
                    self.build_set_entity_queries_recursive(
                        path_clone,
                        event_id,
                        (entity_id, is_event_message),
                        &member.ty,
                        block_timestamp,
                        indexes,
                    );
                }
            }
            Ty::Enum(e) => {
                if e.options.iter().all(
                    |o| {
                        if let Ty::Tuple(t) = &o.ty { t.is_empty() } else { false }
                    },
                ) {
                    return;
                }

                let option = e.options[e.option.unwrap() as usize].clone();

                update_members(
                    &[
                        Member { name: "option".to_string(), ty: Ty::Enum(e.clone()), key: false },
                        Member { name: option.name.clone(), ty: option.ty.clone(), key: false },
                    ],
                    &mut self.query_queue,
                    indexes,
                );

                match &option.ty {
                    // Skip enum options that have no type / member
                    Ty::Tuple(t) if t.is_empty() => {}
                    _ => {
                        let mut path_clone = path.clone();
                        path_clone.push(option.name.clone());
                        self.build_set_entity_queries_recursive(
                            path_clone,
                            event_id,
                            (entity_id, is_event_message),
                            &option.ty,
                            block_timestamp,
                            indexes,
                        );
                    }
                }
            }
            Ty::Tuple(t) => {
                update_members(
                    t.iter()
                        .enumerate()
                        .map(|(idx, member)| Member {
                            name: format!("_{}", idx),
                            ty: member.clone(),
                            key: false,
                        })
                        .collect::<Vec<Member>>()
                        .as_slice(),
                    &mut self.query_queue,
                    indexes,
                );

                for (idx, member) in t.iter().enumerate() {
                    let mut path_clone = path.clone();
                    path_clone.push(format!("_{}", idx));
                    self.build_set_entity_queries_recursive(
                        path_clone,
                        event_id,
                        (entity_id, is_event_message),
                        member,
                        block_timestamp,
                        indexes,
                    );
                }
            }
            Ty::Array(array) => {
                // delete all previous array elements with the array indexes
                let table_id = path.join("$");
                let mut query =
                    format!("DELETE FROM [{table_id}] WHERE entity_id = ? ", table_id = table_id);
                for idx in 0..indexes.len() {
                    query.push_str(&format!("AND idx_{} = ? ", idx));
                }

                // flatten indexes with entity id
                let mut arguments = vec![Argument::String(entity_id.to_string())];
                arguments.extend(indexes.iter().map(|idx| Argument::Int(*idx)));

                self.query_queue.enqueue(query, arguments);

                // insert the new array elements
                for (idx, member) in array.iter().enumerate() {
                    let mut indexes = indexes.clone();
                    indexes.push(idx as i64);

                    update_members(
                        &[Member { name: "data".to_string(), ty: member.clone(), key: false }],
                        &mut self.query_queue,
                        &indexes,
                    );

                    let mut path_clone = path.clone();
                    path_clone.push("data".to_string());
                    self.build_set_entity_queries_recursive(
                        path_clone,
                        event_id,
                        (entity_id, is_event_message),
                        member,
                        block_timestamp,
                        &indexes,
                    );
                }
            }
            _ => {}
        }
    }

    fn build_delete_entity_queries_recursive(
        &mut self,
        path: Vec<String>,
        entity_id: &str,
        entity: &Ty,
    ) {
        match entity {
            Ty::Struct(s) => {
                let table_id = path.join("$");
                let statement = format!("DELETE FROM [{table_id}] WHERE entity_id = ?");
                self.query_queue
                    .push_front(statement, vec![Argument::String(entity_id.to_string())]);
                for member in s.children.iter() {
                    let mut path_clone = path.clone();
                    path_clone.push(member.name.clone());
                    self.build_delete_entity_queries_recursive(path_clone, entity_id, &member.ty);
                }
            }
            Ty::Enum(e) => {
                if e.options
                    .iter()
                    .all(|o| if let Ty::Tuple(t) = &o.ty { t.is_empty() } else { false })
                {
                    return;
                }

                let table_id = path.join("$");
                let statement = format!("DELETE FROM [{table_id}] WHERE entity_id = ?");
                self.query_queue
                    .push_front(statement, vec![Argument::String(entity_id.to_string())]);

                for child in e.options.iter() {
                    if let Ty::Tuple(t) = &child.ty {
                        if t.is_empty() {
                            continue;
                        }
                    }

                    let mut path_clone = path.clone();
                    path_clone.push(child.name.clone());
                    self.build_delete_entity_queries_recursive(path_clone, entity_id, &child.ty);
                }
            }
            Ty::Array(array) => {
                let table_id = path.join("$");
                let statement = format!("DELETE FROM [{table_id}] WHERE entity_id = ?");
                self.query_queue
                    .push_front(statement, vec![Argument::String(entity_id.to_string())]);

                for member in array.iter() {
                    let mut path_clone = path.clone();
                    path_clone.push("data".to_string());
                    self.build_delete_entity_queries_recursive(path_clone, entity_id, member);
                }
            }
            Ty::Tuple(t) => {
                let table_id = path.join("$");
                let statement = format!("DELETE FROM [{table_id}] WHERE entity_id = ?");
                self.query_queue
                    .push_front(statement, vec![Argument::String(entity_id.to_string())]);

                for (idx, member) in t.iter().enumerate() {
                    let mut path_clone = path.clone();
                    path_clone.push(format!("_{}", idx));
                    self.build_delete_entity_queries_recursive(path_clone, entity_id, member);
                }
            }
            _ => {}
        }
    }

    fn build_model_query(
        &mut self,
        path: Vec<String>,
        model: &Ty,
        model_idx: i64,
        block_timestamp: u64,
        array_idx: usize,
        parent_array_idx: usize,
    ) {
        let table_id = path.join("$");
        let mut indices = Vec::new();

        let mut create_table_query = format!(
            "CREATE TABLE IF NOT EXISTS [{table_id}] (id TEXT NOT NULL, event_id TEXT NOT NULL, \
             entity_id TEXT, event_message_id TEXT, "
        );

        if array_idx > 0 {
            // index columns
            for i in 0..array_idx {
                create_table_query.push_str(&format!("idx_{i} INTEGER NOT NULL, ", i = i));
            }

            // full array id column
            create_table_query.push_str("full_array_id TEXT NOT NULL UNIQUE, ");
        }

        let mut build_member = |name: &str, ty: &Ty, options: &mut Option<Argument>| {
            if let Ok(cairo_type) = Primitive::from_str(&ty.name()) {
                create_table_query
                    .push_str(&format!("external_{name} {}, ", cairo_type.to_sql_type()));
                indices.push(format!(
                    "CREATE INDEX IF NOT EXISTS idx_{table_id}_{name} ON [{table_id}] \
                     (external_{name});"
                ));
            } else if let Ty::Enum(e) = &ty {
                let all_options = e
                    .options
                    .iter()
                    .map(|c| format!("'{}'", c.name))
                    .collect::<Vec<_>>()
                    .join(", ");

                create_table_query.push_str(&format!(
                    "external_{name} TEXT CHECK(external_{name} IN ({all_options})) ",
                ));

                // if we're an array, we could have multiple enum options
                create_table_query.push_str(if array_idx > 0 { ", " } else { "NOT NULL, " });

                indices.push(format!(
                    "CREATE INDEX IF NOT EXISTS idx_{table_id}_{name} ON [{table_id}] \
                     (external_{name});"
                ));

                *options = Some(Argument::String(
                    e.options
                        .iter()
                        .map(|c: &dojo_types::schema::EnumOption| c.name.clone())
                        .collect::<Vec<_>>()
                        .join(",")
                        .to_string(),
                ));
            } else if let Ty::ByteArray(_) = &ty {
                create_table_query.push_str(&format!("external_{name} TEXT, "));
                indices.push(format!(
                    "CREATE INDEX IF NOT EXISTS idx_{table_id}_{name} ON [{table_id}] \
                     (external_{name});"
                ));
            }
        };

        match model {
            Ty::Struct(s) => {
                for (member_idx, member) in s.children.iter().enumerate() {
                    let name = member.name.clone();
                    let mut options = None; // TEMP: doesnt support complex enums yet

                    build_member(&name, &member.ty, &mut options);

                    // NOTE: this might cause some errors to fail silently
                    // due to the ignore clause. check migrations for type_enum check
                    let statement = "INSERT OR IGNORE INTO model_members (id, model_id, \
                                     model_idx, member_idx, name, type, type_enum, enum_options, \
                                     key, executed_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)";

                    let arguments = vec![
                        Argument::String(table_id.clone()),
                        // TEMP: this is temporary until the model hash is precomputed
                        Argument::String(format!(
                            "{:#x}",
                            get_selector_from_name(&path[0].clone()).unwrap()
                        )),
                        Argument::Int(model_idx),
                        Argument::Int(member_idx as i64),
                        Argument::String(name),
                        Argument::String(member.ty.name()),
                        Argument::String(member.ty.as_ref().into()),
                        options.unwrap_or(Argument::Null),
                        Argument::Bool(member.key),
                        Argument::String(utc_dt_string_from_timestamp(block_timestamp)),
                    ];

                    self.query_queue.enqueue(statement, arguments);
                }
            }
            Ty::Tuple(tuple) => {
                for (idx, member) in tuple.iter().enumerate() {
                    let mut options = None; // TEMP: doesnt support complex enums yet

                    build_member(&format!("_{}", idx), member, &mut options);

                    let statement = "INSERT OR IGNORE INTO model_members (id, model_id, \
                                     model_idx, member_idx, name, type, type_enum, enum_options, \
                                     key, executed_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)";
                    let arguments = vec![
                        Argument::String(table_id.clone()),
                        // TEMP: this is temporary until the model hash is precomputed
                        Argument::String(format!(
                            "{:#x}",
                            get_selector_from_name(&path[0].clone()).unwrap()
                        )),
                        Argument::Int(model_idx),
                        Argument::Int(idx as i64),
                        Argument::String(format!("_{}", idx)),
                        Argument::String(member.name()),
                        Argument::String(member.as_ref().into()),
                        options.unwrap_or(Argument::Null),
                        // NOTE: should we consider the case where
                        // a tuple is used as a key? should its members be keys?
                        Argument::Bool(false),
                        Argument::String(utc_dt_string_from_timestamp(block_timestamp)),
                    ];

                    self.query_queue.enqueue(statement, arguments);
                }
            }
            Ty::Array(array) => {
                let mut options = None; // TEMP: doesnt support complex enums yet
                let ty = &array[0];
                build_member("data", ty, &mut options);

                let statement = "INSERT OR IGNORE INTO model_members (id, model_id, model_idx, \
                                 member_idx, name, type, type_enum, enum_options, key, \
                                 executed_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)";
                let arguments = vec![
                    Argument::String(table_id.clone()),
                    // TEMP: this is temporary until the model hash is precomputed
                    Argument::String(format!(
                        "{:#x}",
                        get_selector_from_name(&path[0].clone()).unwrap()
                    )),
                    Argument::Int(model_idx),
                    Argument::Int(0),
                    Argument::String("data".to_string()),
                    Argument::String(ty.name()),
                    Argument::String(ty.as_ref().into()),
                    options.unwrap_or(Argument::Null),
                    Argument::Bool(false),
                    Argument::String(utc_dt_string_from_timestamp(block_timestamp)),
                ];

                self.query_queue.enqueue(statement, arguments);
            }
            Ty::Enum(e) => {
                for (idx, child) in e
                    .options
                    .iter()
                    .chain(vec![&EnumOption {
                        name: "option".to_string(),
                        ty: Ty::Enum(e.clone()),
                    }])
                    .enumerate()
                {
                    // Skip enum options that have no type / member
                    if let Ty::Tuple(tuple) = &child.ty {
                        if tuple.is_empty() {
                            continue;
                        }
                    }

                    let mut options = None; // TEMP: doesnt support complex enums yet
                    build_member(&child.name, &child.ty, &mut options);

                    let statement = "INSERT OR IGNORE INTO model_members (id, model_id, \
                                     model_idx, member_idx, name, type, type_enum, enum_options, \
                                     key, executed_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)";
                    let arguments = vec![
                        Argument::String(table_id.clone()),
                        // TEMP: this is temporary until the model hash is precomputed
                        Argument::String(format!(
                            "{:#x}",
                            get_selector_from_name(&path[0].clone()).unwrap()
                        )),
                        Argument::Int(model_idx),
                        Argument::Int(idx as i64),
                        Argument::String(child.name.clone()),
                        Argument::String(child.ty.name()),
                        Argument::String(child.ty.as_ref().into()),
                        options.unwrap_or(Argument::Null),
                        Argument::Bool(false),
                        Argument::String(utc_dt_string_from_timestamp(block_timestamp)),
                    ];

                    self.query_queue.enqueue(statement, arguments);
                }
            }
            _ => {}
        }

        create_table_query.push_str("executed_at DATETIME NOT NULL, ");
        create_table_query.push_str("created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP, ");
        create_table_query.push_str("updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP, ");

        // If this is not the Model's root table, create a reference to the parent.
        if path.len() > 1 {
            let parent_table_id = path[..path.len() - 1].join("$");

            create_table_query.push_str("FOREIGN KEY (id");
            for i in 0..parent_array_idx {
                create_table_query.push_str(&format!(", idx_{i}", i = i));
            }
            create_table_query.push_str(&format!(
                ") REFERENCES [{parent_table_id}] (id",
                parent_table_id = parent_table_id
            ));
            for i in 0..parent_array_idx {
                create_table_query.push_str(&format!(", idx_{i}", i = i));
            }
            create_table_query.push_str(") ON DELETE CASCADE, ");
        };

        create_table_query.push_str("PRIMARY KEY (id");
        for i in 0..array_idx {
            create_table_query.push_str(&format!(", idx_{i}", i = i));
        }
        create_table_query.push_str("), ");

        create_table_query.push_str("FOREIGN KEY (entity_id) REFERENCES entities(id), ");
        // create_table_query.push_str("FOREIGN KEY (event_id) REFERENCES events(id), ");
        create_table_query
            .push_str("FOREIGN KEY (event_message_id) REFERENCES event_messages(id));");

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
