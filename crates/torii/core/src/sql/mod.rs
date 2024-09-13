use std::collections::HashMap;
use std::convert::TryInto;
use std::str::FromStr;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use chrono::Utc;
use dojo_types::primitive::Primitive;
use dojo_types::schema::{EnumOption, Member, Struct, Ty};
use dojo_world::contracts::abi::model::Layout;
use dojo_world::contracts::naming::compute_selector_from_names;
use dojo_world::metadata::WorldMetadata;
use query_queue::{Argument, BrokerMessage, DeleteEntityQuery, QueryQueue, QueryType};
use sqlx::pool::PoolConnection;
use sqlx::{Pool, Sqlite};
use starknet::core::types::{Event, Felt, InvokeTransaction, Transaction};
use starknet_crypto::poseidon_hash_many;
use tracing::{debug, warn};
use utils::felts_sql_string;

use crate::cache::{Model, ModelCache};
use crate::types::{
    ErcContract, Event as EventEmitted, EventMessage as EventMessageUpdated,
    Model as ModelRegistered,
};
use crate::utils::{must_utc_datetime_from_timestamp, utc_dt_string_from_timestamp};

type IsEventMessage = bool;
type IsStoreUpdate = bool;

pub const WORLD_CONTRACT_TYPE: &str = "WORLD";
pub const FELT_DELIMITER: &str = "/";

pub mod erc;
pub mod query_queue;
#[cfg(test)]
#[path = "test.rs"]
mod test;
pub mod utils;

#[derive(Debug)]
pub struct Sql {
    world_address: Felt,
    pub pool: Pool<Sqlite>,
    pub query_queue: QueryQueue,
    model_cache: Arc<ModelCache>,
}

#[derive(Debug, Clone)]
pub struct Cursors {
    pub cursor_map: HashMap<Felt, Felt>,
    pub last_pending_block_tx: Option<Felt>,
    pub head: Option<u64>,
}

impl Clone for Sql {
    fn clone(&self) -> Self {
        Self {
            world_address: self.world_address,
            pool: self.pool.clone(),
            query_queue: QueryQueue::new(self.pool.clone()),
            model_cache: self.model_cache.clone(),
        }
    }
}

impl Sql {
    pub async fn new(
        pool: Pool<Sqlite>,
        world_address: Felt,
        erc_contracts: &HashMap<Felt, ErcContract>,
    ) -> Result<Self> {
        let mut query_queue = QueryQueue::new(pool.clone());

        query_queue.enqueue(
            "INSERT OR IGNORE INTO contracts (id, contract_address, contract_type) VALUES (?, ?, \
             ?)",
            vec![
                Argument::FieldElement(world_address),
                Argument::FieldElement(world_address),
                Argument::String(WORLD_CONTRACT_TYPE.to_string()),
            ],
            QueryType::Other,
        );

        for contract in erc_contracts.values() {
            query_queue.enqueue(
                "INSERT OR IGNORE INTO contracts (id, contract_address, contract_type) VALUES (?, \
                 ?, ?)",
                vec![
                    Argument::FieldElement(contract.contract_address),
                    Argument::FieldElement(contract.contract_address),
                    Argument::String(contract.r#type.to_string()),
                ],
                QueryType::Other,
            );
        }

        query_queue.execute_all().await?;

        Ok(Self {
            pool: pool.clone(),
            world_address,
            query_queue,
            model_cache: Arc::new(ModelCache::new(pool)),
        })
    }

    pub fn merge(&mut self, other: Sql) -> Result<()> {
        // Merge query queue
        self.query_queue.queue.extend(other.query_queue.queue);
        self.query_queue.publish_queue.extend(other.query_queue.publish_queue);

        // This should never happen
        if self.world_address != other.world_address {
            warn!(
                "Merging Sql instances with different world addresses: {} and {}",
                self.world_address, other.world_address
            );
        }

        Ok(())
    }

    pub async fn head(&self, contract: Felt) -> Result<(u64, Option<Felt>, Option<Felt>)> {
        let mut conn: PoolConnection<Sqlite> = self.pool.acquire().await?;
        let indexer_query =
            sqlx::query_as::<_, (Option<i64>, Option<String>, Option<String>, String)>(
                "SELECT head, last_pending_block_contract_tx, last_pending_block_tx, \
                 contract_type FROM contracts WHERE id = ?",
            )
            .bind(format!("{:#x}", contract));

        let indexer: (Option<i64>, Option<String>, Option<String>, String) =
            indexer_query.fetch_one(&mut *conn).await?;
        Ok((
            indexer.0.map(|h| h.try_into().expect("doesn't fit in u64")).unwrap_or(0),
            indexer.1.map(|f| Felt::from_str(&f)).transpose()?,
            indexer.2.map(|f| Felt::from_str(&f)).transpose()?,
        ))
    }

    pub fn set_head(&mut self, contract: Felt, head: u64) {
        let head = Argument::Int(head.try_into().expect("doesn't fit in u64"));
        let id = Argument::FieldElement(contract);
        self.query_queue.enqueue(
            "UPDATE contracts SET head = ? WHERE id = ?",
            vec![head, id],
            QueryType::Other,
        );
    }

    pub fn set_last_pending_block_contract_tx(
        &mut self,
        contract: Felt,
        last_pending_block_contract_tx: Option<Felt>,
    ) {
        let last_pending_block_contract_tx = if let Some(f) = last_pending_block_contract_tx {
            Argument::String(format!("{:#x}", f))
        } else {
            Argument::Null
        };

        let id = Argument::FieldElement(contract);

        self.query_queue.enqueue(
            "UPDATE contracts SET last_pending_block_contract_tx = ? WHERE id = ?",
            vec![last_pending_block_contract_tx, id],
            QueryType::Other,
        );
    }

    pub fn set_last_pending_block_tx(&mut self, last_pending_block_tx: Option<Felt>) {
        let last_pending_block_tx = if let Some(f) = last_pending_block_tx {
            Argument::String(format!("{:#x}", f))
        } else {
            Argument::Null
        };

        self.query_queue.enqueue(
            "UPDATE contracts SET last_pending_block_tx = ? WHERE 1=1",
            vec![last_pending_block_tx],
            QueryType::Other,
        )
    }

    pub(crate) async fn cursors(&self) -> Result<Cursors> {
        let mut conn: PoolConnection<Sqlite> = self.pool.acquire().await?;
        let cursors = sqlx::query_as::<_, (String, String)>(
            "SELECT contract_address, last_pending_block_contract_tx FROM contracts WHERE \
             last_pending_block_contract_tx IS NOT NULL",
        )
        .fetch_all(&mut *conn)
        .await?;

        let (head, last_pending_block_tx) = sqlx::query_as::<_, (Option<i64>, Option<String>)>(
            "SELECT head, last_pending_block_tx FROM contracts WHERE 1=1",
        )
        .fetch_one(&mut *conn)
        .await?;

        let head = head.map(|h| h.try_into().expect("doesn't fit in u64"));
        let last_pending_block_tx =
            last_pending_block_tx.map(|t| Felt::from_str(&t).expect("its a valid felt"));
        Ok(Cursors {
            cursor_map: cursors
                .into_iter()
                .map(|(c, t)| {
                    (
                        Felt::from_str(&c).expect("its a valid felt"),
                        Felt::from_str(&t).expect("its a valid felt"),
                    )
                })
                .collect(),
            last_pending_block_tx,
            head,
        })
    }

    pub fn update_cursors(
        &mut self,
        head: u64,
        last_pending_block_tx: Option<Felt>,
        cursor_map: HashMap<Felt, Felt>,
    ) {
        let head = Argument::Int(head.try_into().expect("doesn't fit in u64"));
        let last_pending_block_tx = if let Some(f) = last_pending_block_tx {
            Argument::String(format!("{:#x}", f))
        } else {
            Argument::Null
        };

        self.query_queue.enqueue(
            "UPDATE contracts SET head = ?, last_pending_block_tx = ? WHERE 1=1",
            vec![head, last_pending_block_tx],
            QueryType::Other,
        );

        for cursor in cursor_map {
            let tx = Argument::FieldElement(cursor.1);
            let contract = Argument::FieldElement(cursor.0);

            self.query_queue.enqueue(
                "UPDATE contracts SET last_pending_block_contract_tx = ? WHERE id = ?",
                vec![tx, contract],
                QueryType::Other,
            );
        }
    }

    // For a given contract address, sets head to the passed value and sets
    // last_pending_block_contract_tx and last_pending_block_tx to null
    pub fn reset_cursors(&mut self, head: u64) {
        let head = Argument::Int(head.try_into().expect("doesn't fit in u64"));
        self.query_queue.enqueue(
            "UPDATE contracts SET head = ?, last_pending_block_contract_tx = ?, \
             last_pending_block_tx = ? WHERE 1=1",
            vec![head, Argument::Null, Argument::Null],
            QueryType::Other,
        );
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
        let namespaced_name = format!("{}-{}", namespace, model.name());

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
            vec![namespaced_name.clone()],
            &mut model_idx,
            block_timestamp,
            &mut 0,
            &mut 0,
        );

        // we set the model in the cache directly
        // because entities might be using it before the query queue is processed
        self.model_cache
            .set(
                selector,
                Model {
                    namespace: namespace.to_string(),
                    name: model.name().to_string(),
                    selector,
                    class_hash,
                    contract_address,
                    packed_size,
                    unpacked_size,
                    layout,
                    // we need to update the name of the struct to include the namespace
                    schema: Ty::Struct(Struct {
                        name: namespaced_name,
                        children: model.as_struct().unwrap().children.clone(),
                    }),
                },
            )
            .await;
        self.query_queue.push_publish(BrokerMessage::ModelRegistered(model_registered));

        Ok(())
    }

    pub async fn set_entity(
        &mut self,
        entity: Ty,
        event_id: &str,
        block_timestamp: u64,
        entity_id: Felt,
        model_id: Felt,
        keys_str: Option<&str>,
    ) -> Result<()> {
        let namespaced_name = entity.name();

        let entity_id = format!("{:#x}", entity_id);
        let model_id = format!("{:#x}", model_id);

        let insert_entities = if keys_str.is_some() {
            "INSERT INTO entities (id, event_id, executed_at, keys) VALUES (?, ?, ?, ?) ON \
             CONFLICT(id) DO UPDATE SET updated_at=CURRENT_TIMESTAMP, \
             executed_at=EXCLUDED.executed_at, event_id=EXCLUDED.event_id, keys=EXCLUDED.keys \
             RETURNING *"
        } else {
            "INSERT INTO entities (id, event_id, executed_at) VALUES (?, ?, ?) ON CONFLICT(id) DO \
             UPDATE SET updated_at=CURRENT_TIMESTAMP, executed_at=EXCLUDED.executed_at, \
             event_id=EXCLUDED.event_id RETURNING *"
        };

        let mut arguments = vec![
            Argument::String(entity_id.clone()),
            Argument::String(event_id.to_string()),
            Argument::String(utc_dt_string_from_timestamp(block_timestamp)),
        ];

        if let Some(keys) = keys_str {
            arguments.push(Argument::String(keys.to_string()));
        }

        self.query_queue.enqueue(insert_entities, arguments, QueryType::SetEntity(entity.clone()));

        self.query_queue.enqueue(
            "INSERT INTO entity_model (entity_id, model_id) VALUES (?, ?) ON CONFLICT(entity_id, \
             model_id) DO NOTHING",
            vec![Argument::String(entity_id.clone()), Argument::String(model_id.clone())],
            QueryType::Other,
        );

        let path = vec![namespaced_name];
        self.build_set_entity_queries_recursive(
            path,
            event_id,
            (&entity_id, false),
            (&entity, keys_str.is_none()),
            block_timestamp,
            &vec![],
        );

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
            QueryType::Other,
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

        self.query_queue.push_publish(BrokerMessage::EventMessageUpdated(event_message_updated));

        Ok(())
    }

    pub async fn delete_entity(
        &mut self,
        entity_id: Felt,
        model_id: Felt,
        entity: Ty,
        event_id: &str,
        block_timestamp: u64,
    ) -> Result<()> {
        let entity_id = format!("{:#x}", entity_id);
        let path = vec![entity.name()];
        // delete entity models data
        self.build_delete_entity_queries_recursive(path, &entity_id, &entity);

        self.query_queue.enqueue(
            "DELETE FROM entity_model WHERE entity_id = ? AND model_id = ?",
            vec![Argument::String(entity_id.clone()), Argument::String(format!("{:#x}", model_id))],
            QueryType::DeleteEntity(DeleteEntityQuery {
                entity_id: entity_id.clone(),
                event_id: event_id.to_string(),
                block_timestamp: utc_dt_string_from_timestamp(block_timestamp),
                ty: entity.clone(),
            }),
        );

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
            QueryType::Other,
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

        self.query_queue.enqueue(statement, arguments, QueryType::Other);

        Ok(())
    }

    pub async fn model(&self, selector: Felt) -> Result<Model> {
        self.model_cache.model(&selector).await.map_err(|e| e.into())
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
            QueryType::Other,
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
            QueryType::Other,
        );

        let emitted = EventEmitted {
            id: event_id.to_string(),
            keys: felts_sql_string(&event.keys),
            data: felts_sql_string(&event.data),
            transaction_hash: format!("{:#x}", transaction_hash),
            created_at: Utc::now(),
            executed_at: must_utc_datetime_from_timestamp(block_timestamp),
        };

        self.query_queue.push_publish(BrokerMessage::EventEmitted(emitted));
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
        entity: (&Ty, IsStoreUpdate),
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

                query_queue.enqueue(statement, arguments, QueryType::Other);
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

                self.query_queue.enqueue(query, arguments, QueryType::Other);

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
                self.query_queue.enqueue(
                    statement,
                    vec![Argument::String(entity_id.to_string())],
                    QueryType::Other,
                );
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
                self.query_queue.enqueue(
                    statement,
                    vec![Argument::String(entity_id.to_string())],
                    QueryType::Other,
                );

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
                self.query_queue.enqueue(
                    statement,
                    vec![Argument::String(entity_id.to_string())],
                    QueryType::Other,
                );

                for member in array.iter() {
                    let mut path_clone = path.clone();
                    path_clone.push("data".to_string());
                    self.build_delete_entity_queries_recursive(path_clone, entity_id, member);
                }
            }
            Ty::Tuple(t) => {
                let table_id = path.join("$");
                let statement = format!("DELETE FROM [{table_id}] WHERE entity_id = ?");
                self.query_queue.enqueue(
                    statement,
                    vec![Argument::String(entity_id.to_string())],
                    QueryType::Other,
                );

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

                    self.query_queue.enqueue(statement, arguments, QueryType::Other);
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

                    self.query_queue.enqueue(statement, arguments, QueryType::Other);
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

                self.query_queue.enqueue(statement, arguments, QueryType::Other);
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

                    self.query_queue.enqueue(statement, arguments, QueryType::Other);
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

        self.query_queue.enqueue(create_table_query, vec![], QueryType::Other);

        indices.iter().for_each(|s| {
            self.query_queue.enqueue(s, vec![], QueryType::Other);
        });
    }

    /// Execute all queries in the queue
    pub async fn execute(&mut self) -> Result<()> {
        debug!("Executing {} queries from the queue", self.query_queue.queue.len());
        self.query_queue.execute_all().await?;

        Ok(())
    }
}
