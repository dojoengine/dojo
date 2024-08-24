use std::convert::TryInto;
use std::str::FromStr;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use chrono::Utc;
use dojo_types::primitive::Primitive;
use dojo_types::schema::{EnumOption, Member, Struct, Ty};
use dojo_world::contracts::abi::model::Layout;
use dojo_world::contracts::naming::{compute_selector_from_names, compute_selector_from_tag};
use dojo_world::metadata::WorldMetadata;
use sqlx::pool::PoolConnection;
use sqlx::{Pool, Row, Sqlite};
use starknet::core::types::{Event, Felt, InvokeTransaction, Transaction};
use starknet_crypto::poseidon_hash_many;

use crate::cache::{Model, ModelCache};
use crate::query_queue::{Argument, QueryQueue};
use crate::simple_broker::SimpleBroker;
use crate::types::{
    Entity as EntityUpdated, Event as EventEmitted, EventMessage as EventMessageUpdated,
    Model as ModelRegistered,
};
use crate::utils::{must_utc_datetime_from_timestamp, utc_dt_string_from_timestamp};
use crate::World;

type IsEventMessage = bool;
type IsStoreUpdateMember = bool;

pub const FELT_DELIMITER: &str = "/";

#[cfg(test)]
#[path = "sql_test.rs"]
mod test;

#[derive(Debug, Clone)]
pub struct Sql {
    world_address: Felt,
    pub pool: Pool<Sqlite>,
    query_queue: QueryQueue,
    model_cache: Arc<ModelCache>,
}

impl Sql {
    pub async fn new(pool: Pool<Sqlite>, world_address: Felt) -> Result<Self> {
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

        Ok(Self {
            pool: pool.clone(),
            world_address,
            query_queue,
            model_cache: Arc::new(ModelCache::new(pool)),
        })
    }

    pub async fn head(&self) -> Result<(u64, Option<Felt>)> {
        let mut conn: PoolConnection<Sqlite> = self.pool.acquire().await?;
        let indexer_query = sqlx::query_as::<_, (i64, Option<String>)>(
            "SELECT head, pending_block_tx FROM indexers WHERE id = ?",
        )
        .bind(format!("{:#x}", self.world_address));

        let indexer: (i64, Option<String>) = indexer_query.fetch_one(&mut *conn).await?;
        Ok((
            indexer.0.try_into().expect("doesn't fit in u64"),
            indexer.1.map(|f| Felt::from_str(&f)).transpose()?,
        ))
    }

    pub fn set_head(&mut self, head: u64, pending_block_tx: Option<Felt>) {
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
        namespace: &str,
        model: Ty,
        layout: Layout,
        class_hash: Felt,
        contract_address: Felt,
        packed_size: u32,
        unpacked_size: u32,
        block_timestamp: u64,
    ) -> Result<()> {
        let selector = compute_selector_from_names(namespace, &model.name());

        let insert_models =
            "INSERT INTO models (id, namespace, name, class_hash, contract_address, layout, \
             packed_size, unpacked_size, executed_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?) ON \
             CONFLICT(id) DO UPDATE SET contract_address=EXCLUDED.contract_address, \
             class_hash=EXCLUDED.class_hash, layout=EXCLUDED.layout, \
             packed_size=EXCLUDED.packed_size, unpacked_size=EXCLUDED.unpacked_size, \
             executed_at=EXCLUDED.executed_at RETURNING *";
        let model_registered: ModelRegistered = sqlx::query_as(insert_models)
            // this is temporary until the model hash is precomputed
            .bind(format!("{:#x}", selector))
            .bind(namespace)
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
            selector,
            &model,
            vec![format!("{}-{}", namespace, model.name())],
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

        let namespaced_name = entity.name();
        let (model_namespace, model_name) = namespaced_name.split_once('-').unwrap();

        let entity_id = format!("{:#x}", poseidon_hash_many(&keys));
        let model_id = format!("{:#x}", compute_selector_from_names(model_namespace, model_name));

        self.query_queue.enqueue(
            "INSERT INTO entity_model (entity_id, model_id) VALUES (?, ?) ON CONFLICT(entity_id, \
             model_id) DO NOTHING",
            vec![Argument::String(entity_id.clone()), Argument::String(model_id.clone())],
        );

        let keys_str = felts_sql_string(&keys);
        let insert_entities = "INSERT INTO entities (id, keys, event_id, executed_at) VALUES (?, \
                               ?, ?, ?) ON CONFLICT(id) DO UPDATE SET \
                               updated_at=CURRENT_TIMESTAMP, executed_at=EXCLUDED.executed_at, \
                               event_id=EXCLUDED.event_id RETURNING *";
        let mut entity_updated: EntityUpdated = sqlx::query_as(insert_entities)
            .bind(&entity_id)
            .bind(&keys_str)
            .bind(event_id)
            .bind(utc_dt_string_from_timestamp(block_timestamp))
            .fetch_one(&self.pool)
            .await?;

        entity_updated.updated_model = Some(entity.clone());

        let path = vec![namespaced_name];
        self.build_set_entity_queries_recursive(
            path,
            event_id,
            (&entity_id, false),
            (&entity, false),
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

        let namespaced_name = entity.name();
        let (model_namespace, model_name) = namespaced_name.split_once('-').unwrap();

        let entity_id = format!("{:#x}", poseidon_hash_many(&keys));
        let model_id = format!("{:#x}", compute_selector_from_names(model_namespace, model_name));

        self.query_queue.enqueue(
            "INSERT INTO event_model (entity_id, model_id) VALUES (?, ?) ON CONFLICT(entity_id, \
             model_id) DO NOTHING",
            vec![Argument::String(entity_id.clone()), Argument::String(model_id.clone())],
        );

        let keys_str = felts_sql_string(&keys);
        let insert_entities = "INSERT INTO event_messages (id, keys, event_id, executed_at) \
                               VALUES (?, ?, ?, ?) ON CONFLICT(id) DO UPDATE SET \
                               updated_at=CURRENT_TIMESTAMP, executed_at=EXCLUDED.executed_at, \
                               event_id=EXCLUDED.event_id RETURNING *";
        let mut event_message_updated: EventMessageUpdated = sqlx::query_as(insert_entities)
            .bind(&entity_id)
            .bind(&keys_str)
            .bind(event_id)
            .bind(utc_dt_string_from_timestamp(block_timestamp))
            .fetch_one(&self.pool)
            .await?;

        event_message_updated.updated_model = Some(entity.clone());

        let path = vec![namespaced_name];
        self.build_set_entity_queries_recursive(
            path,
            event_id,
            (&entity_id, true),
            (&entity, false),
            block_timestamp,
            &vec![],
        );
        self.query_queue.execute_all().await?;

        SimpleBroker::publish(event_message_updated);

        Ok(())
    }

    pub async fn set_model_member(
        &mut self,
        model_tag: &str,
        entity_id: Felt,
        is_event_message: bool,
        member: &Member,
        event_id: &str,
        block_timestamp: u64,
    ) -> Result<()> {
        let entity_id = format!("{:#x}", entity_id);
        let path = vec![model_tag.to_string()];

        let wrapped_ty =
            Ty::Struct(Struct { name: model_tag.to_string(), children: vec![member.clone()] });

        // update model member
        self.build_set_entity_queries_recursive(
            path,
            event_id,
            (&entity_id, is_event_message),
            (&wrapped_ty, true),
            block_timestamp,
            &vec![],
        );
        self.query_queue.execute_all().await?;

        let mut update_entity = sqlx::query_as::<_, EntityUpdated>(
            "UPDATE entities SET updated_at=CURRENT_TIMESTAMP, executed_at=?, event_id=? WHERE id \
             = ? RETURNING *",
        )
        .bind(utc_dt_string_from_timestamp(block_timestamp))
        .bind(event_id)
        .bind(entity_id)
        .fetch_one(&self.pool)
        .await?;

        update_entity.updated_model = Some(wrapped_ty);

        SimpleBroker::publish(update_entity);

        Ok(())
    }

    pub async fn delete_entity(
        &mut self,
        entity_id: Felt,
        entity: Ty,
        event_id: &str,
        block_timestamp: u64,
    ) -> Result<()> {
        let entity_id = format!("{:#x}", entity_id);
        let path = vec![entity.name()];
        // delete entity models data
        self.build_delete_entity_queries_recursive(path, &entity_id, &entity);
        self.query_queue.execute_all().await?;

        let deleted_entity_model =
            sqlx::query("DELETE FROM entity_model WHERE entity_id = ? AND model_id = ?")
                .bind(&entity_id)
                .bind(format!("{:#x}", compute_selector_from_tag(&entity.name())))
                .execute(&self.pool)
                .await?;
        if deleted_entity_model.rows_affected() == 0 {
            // fail silently. we have no entity-model relation to delete.
            // this can happen if a entity model that doesnt exist
            // got deleted
            return Ok(());
        }

        let mut update_entity = sqlx::query_as::<_, EntityUpdated>(
            "UPDATE entities SET updated_at=CURRENT_TIMESTAMP, executed_at=?, event_id=? WHERE id \
             = ? RETURNING *",
        )
        .bind(utc_dt_string_from_timestamp(block_timestamp))
        .bind(event_id)
        .bind(&entity_id)
        .fetch_one(&self.pool)
        .await?;
        update_entity.updated_model = Some(entity.clone());

        let models_count =
            sqlx::query_scalar::<_, u32>("SELECT count(*) FROM entity_model WHERE entity_id = ?")
                .bind(&entity_id)
                .fetch_one(&self.pool)
                .await?;

        if models_count == 0 {
            // delete entity
            sqlx::query("DELETE FROM entities WHERE id = ?")
                .bind(&entity_id)
                .execute(&self.pool)
                .await?;

            update_entity.deleted = true;
        }

        SimpleBroker::publish(update_entity);
        Ok(())
    }

    pub fn set_metadata(&mut self, resource: &Felt, uri: &str, block_timestamp: u64) {
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
        resource: &Felt,
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

    pub async fn model(&self, selector: Felt) -> Result<Model> {
        self.model_cache.model(&selector).await.map_err(|e| e.into())
    }

    /// Retrieves the keys definition for a given model.
    /// The key definition is currently implemented as (`name`, `type`).
    pub async fn get_entity_keys_def(&self, model_tag: &str) -> Result<Vec<(String, String)>> {
        let query = sqlx::query_as::<_, (String, String)>(
            "SELECT name, type FROM model_members WHERE id = ? AND key = true",
        )
        .bind(model_tag);

        let mut conn: PoolConnection<Sqlite> = self.pool.acquire().await?;
        let rows: Vec<(String, String)> = query.fetch_all(&mut *conn).await?;
        Ok(rows.iter().map(|(name, ty)| (name.to_string(), ty.to_string())).collect())
    }

    /// Retrieves the keys for a given entity.
    /// The keys are returned in the same order as the keys definition.
    pub async fn get_entity_keys(&self, entity_id: Felt, model_tag: &str) -> Result<Vec<Felt>> {
        let entity_id = format!("{:#x}", entity_id);
        let keys_def = self.get_entity_keys_def(model_tag).await?;

        let keys_names =
            keys_def.iter().map(|(name, _)| format!("external_{}", name)).collect::<Vec<String>>();

        let sql = format!("SELECT {} FROM [{}] WHERE id = ?", keys_names.join(", "), model_tag);
        let query = sqlx::query(sql.as_str()).bind(entity_id);

        let mut conn: PoolConnection<Sqlite> = self.pool.acquire().await?;

        let mut keys: Vec<Felt> = vec![];
        let result = query.fetch_all(&mut *conn).await?;

        for row in result {
            for (i, _) in row.columns().iter().enumerate() {
                let value: String = row.try_get(i)?;
                keys.push(Felt::from_hex(&value)?);
            }
        }

        Ok(keys)
    }

    pub async fn does_entity_exist(&self, model: String, key: Felt) -> Result<bool> {
        let sql = format!("SELECT COUNT(*) FROM [{model}] WHERE id = ?");

        let count: i64 =
            sqlx::query_scalar(&sql).bind(format!("{:#x}", key)).fetch_one(&self.pool).await?;

        Ok(count > 0)
    }

    pub async fn entities(&self, model: String) -> Result<Vec<Vec<Felt>>> {
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
                    Argument::FieldElement(Felt::ZERO), // has no max_fee
                    Argument::String("".to_string()),   // has no signature
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
        transaction_hash: Felt,
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

    #[allow(clippy::too_many_arguments)]
    fn build_register_queries_recursive(
        &mut self,
        selector: Felt,
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
            selector,
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
                selector,
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
        entity_id: (&str, IsEventMessage),
        entity: (&Ty, IsStoreUpdateMember),
        block_timestamp: u64,
        indexes: &Vec<i64>,
    ) {
        let (entity_id, is_event_message) = entity_id;
        let (entity, is_store_update_member) = entity;

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
                let statement = if is_store_update_member && indexes.is_empty() {
                    arguments.push(Argument::String(if is_event_message {
                        "event:".to_string() + entity_id
                    } else {
                        entity_id.to_string()
                    }));

                    // row has to exist. update it directly
                    format!(
                        "UPDATE [{table_id}] SET {updates} WHERE id = ?",
                        table_id = table_id,
                        updates = columns
                            .iter()
                            .zip(placeholders.iter())
                            .map(|(column, placeholder)| format!("{} = {}", column, placeholder))
                            .collect::<Vec<String>>()
                            .join(", ")
                    )
                } else {
                    format!(
                        "INSERT OR REPLACE INTO [{table_id}] ({}) VALUES ({})",
                        columns.join(","),
                        placeholders.join(",")
                    )
                };

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
                        (&member.ty, is_store_update_member),
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
                            (&option.ty, is_store_update_member),
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
                        (member, is_store_update_member),
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
                        (member, is_store_update_member),
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

    #[allow(clippy::too_many_arguments)]
    fn build_model_query(
        &mut self,
        selector: Felt,
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
                    "CREATE INDEX IF NOT EXISTS [idx_{table_id}_{name}] ON [{table_id}] \
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
                    "CREATE INDEX IF NOT EXISTS [idx_{table_id}_{name}] ON [{table_id}] \
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
                    "CREATE INDEX IF NOT EXISTS [idx_{table_id}_{name}] ON [{table_id}] \
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
                        Argument::String(format!("{:#x}", selector)),
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
                        Argument::String(format!("{:#x}", selector)),
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
                    Argument::String(format!("{:#x}", selector)),
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
                        Argument::String(format!("{:#x}", selector)),
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

fn felts_sql_string(felts: &[Felt]) -> String {
    felts.iter().map(|k| format!("{:#x}", k)).collect::<Vec<String>>().join(FELT_DELIMITER)
        + FELT_DELIMITER
}
