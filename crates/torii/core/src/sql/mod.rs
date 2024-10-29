use std::collections::HashMap;
use std::convert::TryInto;
use std::str::FromStr;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use dojo_types::primitive::Primitive;
use dojo_types::schema::{EnumOption, Member, Struct, Ty};
use dojo_world::contracts::abigen::model::Layout;
use dojo_world::contracts::naming::compute_selector_from_names;
use dojo_world::metadata::world::WorldMetadata;
use sqlx::pool::PoolConnection;
use sqlx::{Pool, Sqlite};
use starknet::core::types::{Event, Felt, InvokeTransaction, Transaction};
use starknet_crypto::poseidon_hash_many;
use tokio::sync::mpsc::UnboundedSender;
use utils::felts_to_sql_string;

use crate::executor::{
    Argument, DeleteEntityQuery, EventMessageQuery, QueryMessage, QueryType, ResetCursorsQuery,
    SetHeadQuery, UpdateCursorsQuery,
};
use crate::types::ContractType;
use crate::utils::utc_dt_string_from_timestamp;

type IsEventMessage = bool;
type IsStoreUpdate = bool;

pub const WORLD_CONTRACT_TYPE: &str = "WORLD";
pub const FELT_DELIMITER: &str = "/";

pub mod cache;
pub mod erc;
pub mod query_queue;
#[cfg(test)]
#[path = "test.rs"]
mod test;
pub mod utils;

use cache::{LocalCache, Model, ModelCache};

#[derive(Debug, Clone)]
pub struct Sql {
    pub pool: Pool<Sqlite>,
    pub executor: UnboundedSender<QueryMessage>,
    model_cache: Arc<ModelCache>,
    // when SQL struct is cloned a empty local_cache is created
    local_cache: LocalCache,
}

#[derive(Debug, Clone)]
pub struct Cursors {
    pub cursor_map: HashMap<Felt, Felt>,
    pub last_pending_block_tx: Option<Felt>,
    pub head: Option<u64>,
}

impl Sql {
    pub async fn new(
        pool: Pool<Sqlite>,
        executor: UnboundedSender<QueryMessage>,
        contracts: &HashMap<Felt, ContractType>,
    ) -> Result<Self> {
        for contract in contracts {
            executor.send(QueryMessage::other(
                "INSERT OR IGNORE INTO contracts (id, contract_address, contract_type) VALUES (?, \
                 ?, ?)"
                    .to_string(),
                vec![
                    Argument::FieldElement(*contract.0),
                    Argument::FieldElement(*contract.0),
                    Argument::String(contract.1.to_string()),
                ],
            ))?;
        }

        let local_cache = LocalCache::new(pool.clone()).await;
        let db = Self {
            pool: pool.clone(),
            executor,
            model_cache: Arc::new(ModelCache::new(pool.clone())),
            local_cache,
        };

        db.execute().await?;

        Ok(db)
    }

    pub async fn head(&self, contract: Felt) -> Result<(u64, Option<Felt>, Option<Felt>)> {
        let indexer_query =
            sqlx::query_as::<_, (Option<i64>, Option<String>, Option<String>, String)>(
                "SELECT head, last_pending_block_contract_tx, last_pending_block_tx, \
                 contract_type FROM contracts WHERE id = ?",
            )
            .bind(format!("{:#x}", contract));

        let indexer: (Option<i64>, Option<String>, Option<String>, String) = indexer_query
            .fetch_one(&self.pool)
            .await
            .with_context(|| format!("Failed to fetch head for contract: {:#x}", contract))?;
        Ok((
            indexer
                .0
                .map(|h| h.try_into().map_err(|_| anyhow!("Head value {} doesn't fit in u64", h)))
                .transpose()?
                .unwrap_or(0),
            indexer.1.map(|f| Felt::from_str(&f)).transpose()?,
            indexer.2.map(|f| Felt::from_str(&f)).transpose()?,
        ))
    }

    pub async fn set_head(
        &mut self,
        head: u64,
        last_block_timestamp: u64,
        world_txns_count: u64,
        contract_address: Felt,
    ) -> Result<()> {
        let head_arg = Argument::Int(
            head.try_into().map_err(|_| anyhow!("Head value {} doesn't fit in i64", head))?,
        );
        let last_block_timestamp_arg =
            Argument::Int(last_block_timestamp.try_into().map_err(|_| {
                anyhow!("Last block timestamp value {} doesn't fit in i64", last_block_timestamp)
            })?);
        let id = Argument::FieldElement(contract_address);

        self.executor.send(QueryMessage::new(
            "UPDATE contracts SET head = ?, last_block_timestamp = ? WHERE id = ?".to_string(),
            vec![head_arg, last_block_timestamp_arg, id],
            QueryType::SetHead(SetHeadQuery {
                head,
                last_block_timestamp,
                txns_count: world_txns_count,
                contract_address,
            }),
        ))?;

        Ok(())
    }

    pub fn set_last_pending_block_contract_tx(
        &mut self,
        contract: Felt,
        last_pending_block_contract_tx: Option<Felt>,
    ) -> Result<()> {
        let last_pending_block_contract_tx = if let Some(f) = last_pending_block_contract_tx {
            Argument::String(format!("{:#x}", f))
        } else {
            Argument::Null
        };

        let id = Argument::FieldElement(contract);

        self.executor.send(QueryMessage::other(
            "UPDATE contracts SET last_pending_block_contract_tx = ? WHERE id = ?".to_string(),
            vec![last_pending_block_contract_tx, id],
        ))?;

        Ok(())
    }

    pub fn set_last_pending_block_tx(&mut self, last_pending_block_tx: Option<Felt>) -> Result<()> {
        let last_pending_block_tx = if let Some(f) = last_pending_block_tx {
            Argument::String(format!("{:#x}", f))
        } else {
            Argument::Null
        };

        self.executor.send(QueryMessage::other(
            "UPDATE contracts SET last_pending_block_tx = ? WHERE 1=1".to_string(),
            vec![last_pending_block_tx],
        ))?;

        Ok(())
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

    // For a given contract address, sets head to the passed value and sets
    // last_pending_block_contract_tx and last_pending_block_tx to null
    pub fn reset_cursors(
        &mut self,
        head: u64,
        cursor_map: HashMap<Felt, (Felt, u64)>,
        last_block_timestamp: u64,
    ) -> Result<()> {
        self.executor.send(QueryMessage::new(
            "".to_string(),
            vec![],
            QueryType::ResetCursors(ResetCursorsQuery {
                cursor_map,
                last_block_timestamp,
                last_block_number: head,
            }),
        ))?;

        Ok(())
    }

    pub fn update_cursors(
        &mut self,
        head: u64,
        last_pending_block_tx: Option<Felt>,
        cursor_map: HashMap<Felt, (Felt, u64)>,
        pending_block_timestamp: u64,
    ) -> Result<()> {
        self.executor.send(QueryMessage::new(
            "".to_string(),
            vec![],
            QueryType::UpdateCursors(UpdateCursorsQuery {
                cursor_map,
                last_pending_block_tx,
                last_block_number: head,
                pending_block_timestamp,
            }),
        ))?;
        Ok(())
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
        let arguments = vec![
            Argument::String(format!("{:#x}", selector)),
            Argument::String(namespace.to_string()),
            Argument::String(model.name().to_string()),
            Argument::String(format!("{class_hash:#x}")),
            Argument::String(format!("{contract_address:#x}")),
            Argument::String(serde_json::to_string(&layout)?),
            Argument::Int(packed_size as i64),
            Argument::Int(unpacked_size as i64),
            Argument::String(utc_dt_string_from_timestamp(block_timestamp)),
        ];
        self.executor.send(QueryMessage::new(
            insert_models.to_string(),
            arguments,
            QueryType::RegisterModel,
        ))?;

        let mut model_idx = 0_i64;
        self.build_register_queries_recursive(
            selector,
            &model,
            vec![namespaced_name.clone()],
            &mut model_idx,
            block_timestamp,
            &mut 0,
            &mut 0,
        )?;

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

        self.executor.send(QueryMessage::new(
            insert_entities.to_string(),
            arguments,
            QueryType::SetEntity(entity.clone()),
        ))?;

        self.executor.send(QueryMessage::other(
            "INSERT INTO entity_model (entity_id, model_id) VALUES (?, ?) ON CONFLICT(entity_id, \
             model_id) DO NOTHING"
                .to_string(),
            vec![Argument::String(entity_id.clone()), Argument::String(model_id.clone())],
        ))?;

        let path = vec![namespaced_name];
        self.build_set_entity_queries_recursive(
            path,
            event_id,
            (&entity_id, false),
            (&entity, keys_str.is_none()),
            block_timestamp,
            &vec![],
        )?;

        Ok(())
    }

    pub async fn set_event_message(
        &mut self,
        entity: Ty,
        event_id: &str,
        block_timestamp: u64,
        is_historical: bool,
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

        let keys_str = felts_to_sql_string(&keys);
        let block_timestamp_str = utc_dt_string_from_timestamp(block_timestamp);

        let insert_entities = "INSERT INTO event_messages (id, keys, event_id, executed_at) \
                               VALUES (?, ?, ?, ?) ON CONFLICT(id) DO UPDATE SET \
                               updated_at=CURRENT_TIMESTAMP, executed_at=EXCLUDED.executed_at, \
                               event_id=EXCLUDED.event_id RETURNING *";
        self.executor.send(QueryMessage::new(
            insert_entities.to_string(),
            vec![
                Argument::String(entity_id.clone()),
                Argument::String(keys_str.clone()),
                Argument::String(event_id.to_string()),
                Argument::String(block_timestamp_str.clone()),
            ],
            QueryType::EventMessage(EventMessageQuery {
                entity_id: entity_id.clone(),
                model_id: model_id.clone(),
                keys_str: keys_str.clone(),
                event_id: event_id.to_string(),
                block_timestamp: block_timestamp_str.clone(),
                ty: entity.clone(),
                is_historical,
            }),
        ))?;

        let path = vec![namespaced_name];
        self.build_set_entity_queries_recursive(
            path,
            event_id,
            (&entity_id, true),
            (&entity, false),
            block_timestamp,
            &vec![],
        )?;

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
        self.build_delete_entity_queries_recursive(path, &entity_id, &entity)?;

        self.executor.send(QueryMessage::new(
            "DELETE FROM entity_model WHERE entity_id = ? AND model_id = ?".to_string(),
            vec![Argument::String(entity_id.clone()), Argument::String(format!("{:#x}", model_id))],
            QueryType::DeleteEntity(DeleteEntityQuery {
                entity_id: entity_id.clone(),
                event_id: event_id.to_string(),
                block_timestamp: utc_dt_string_from_timestamp(block_timestamp),
                ty: entity.clone(),
            }),
        ))?;

        Ok(())
    }

    pub fn set_metadata(&mut self, resource: &Felt, uri: &str, block_timestamp: u64) -> Result<()> {
        let resource = Argument::FieldElement(*resource);
        let uri = Argument::String(uri.to_string());
        let executed_at = Argument::String(utc_dt_string_from_timestamp(block_timestamp));

        self.executor.send(QueryMessage::other(
            "INSERT INTO metadata (id, uri, executed_at) VALUES (?, ?, ?) ON CONFLICT(id) DO \
             UPDATE SET id=excluded.id, executed_at=excluded.executed_at, \
             updated_at=CURRENT_TIMESTAMP"
                .to_string(),
            vec![resource, uri, executed_at],
        ))?;

        Ok(())
    }

    pub fn update_metadata(
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

        self.executor.send(QueryMessage::other(statement, arguments))?;

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
    ) -> Result<()> {
        let id = Argument::String(transaction_id.to_string());

        let transaction_type = match transaction {
            Transaction::Invoke(_) => "INVOKE",
            Transaction::L1Handler(_) => "L1_HANDLER",
            _ => return Ok(()),
        };

        let (transaction_hash, sender_address, calldata, max_fee, signature, nonce) =
            match transaction {
                Transaction::Invoke(InvokeTransaction::V1(invoke_v1_transaction)) => (
                    Argument::FieldElement(invoke_v1_transaction.transaction_hash),
                    Argument::FieldElement(invoke_v1_transaction.sender_address),
                    Argument::String(felts_to_sql_string(&invoke_v1_transaction.calldata)),
                    Argument::FieldElement(invoke_v1_transaction.max_fee),
                    Argument::String(felts_to_sql_string(&invoke_v1_transaction.signature)),
                    Argument::FieldElement(invoke_v1_transaction.nonce),
                ),
                Transaction::L1Handler(l1_handler_transaction) => (
                    Argument::FieldElement(l1_handler_transaction.transaction_hash),
                    Argument::FieldElement(l1_handler_transaction.contract_address),
                    Argument::String(felts_to_sql_string(&l1_handler_transaction.calldata)),
                    Argument::FieldElement(Felt::ZERO), // has no max_fee
                    Argument::String("".to_string()),   // has no signature
                    Argument::FieldElement((l1_handler_transaction.nonce).into()),
                ),
                _ => return Ok(()),
            };

        self.executor.send(QueryMessage::other(
            "INSERT OR IGNORE INTO transactions (id, transaction_hash, sender_address, calldata, \
             max_fee, signature, nonce, transaction_type, executed_at) VALUES (?, ?, ?, ?, ?, ?, \
             ?, ?, ?)"
                .to_string(),
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
        ))?;

        Ok(())
    }

    pub fn store_event(
        &mut self,
        event_id: &str,
        event: &Event,
        transaction_hash: Felt,
        block_timestamp: u64,
    ) -> Result<()> {
        let id = Argument::String(event_id.to_string());
        let keys = Argument::String(felts_to_sql_string(&event.keys));
        let data = Argument::String(felts_to_sql_string(&event.data));
        let hash = Argument::FieldElement(transaction_hash);
        let executed_at = Argument::String(utc_dt_string_from_timestamp(block_timestamp));

        self.executor.send(QueryMessage::new(
            "INSERT OR IGNORE INTO events (id, keys, data, transaction_hash, executed_at) VALUES \
             (?, ?, ?, ?, ?) RETURNING *"
                .to_string(),
            vec![id, keys, data, hash, executed_at],
            QueryType::StoreEvent,
        ))?;

        Ok(())
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
    ) -> Result<()> {
        if let Ty::Enum(e) = model {
            if e.options.iter().all(|o| if let Ty::Tuple(t) = &o.ty { t.is_empty() } else { false })
            {
                return Ok(());
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
        )?;

        let mut build_member = |pathname: &str, member: &Ty| -> Result<()> {
            if let Ty::Primitive(_) = member {
                return Ok(());
            } else if let Ty::ByteArray(_) = member {
                return Ok(());
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
            )?;

            Ok(())
        };

        if let Ty::Struct(s) = model {
            for member in s.children.iter() {
                build_member(&member.name, &member.ty)?;
            }
        } else if let Ty::Tuple(t) = model {
            for (idx, member) in t.iter().enumerate() {
                build_member(format!("_{}", idx).as_str(), member)?;
            }
        } else if let Ty::Array(array) = model {
            let ty = &array[0];
            build_member("data", ty)?;
        } else if let Ty::Enum(e) = model {
            for child in e.options.iter() {
                // Skip enum options that have no type / member
                if let Ty::Tuple(t) = &child.ty {
                    if t.is_empty() {
                        continue;
                    }
                }

                build_member(&child.name, &child.ty)?;
            }
        }

        Ok(())
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
    ) -> Result<()> {
        let (entity_id, is_event_message) = entity_id;
        let (entity, is_store_update_member) = entity;

        let update_members = |members: &[Member],
                              executor: &mut UnboundedSender<QueryMessage>,
                              indexes: &Vec<i64>|
         -> Result<()> {
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

            executor.send(QueryMessage::other(statement, arguments))?;

            Ok(())
        };

        match entity {
            Ty::Struct(s) => {
                update_members(&s.children, &mut self.executor, indexes)?;

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
                    )?;
                }
            }
            Ty::Enum(e) => {
                if e.options.iter().all(
                    |o| {
                        if let Ty::Tuple(t) = &o.ty { t.is_empty() } else { false }
                    },
                ) {
                    return Ok(());
                }

                let option = e.options[e.option.unwrap() as usize].clone();

                update_members(
                    &[
                        Member { name: "option".to_string(), ty: Ty::Enum(e.clone()), key: false },
                        Member { name: option.name.clone(), ty: option.ty.clone(), key: false },
                    ],
                    &mut self.executor,
                    indexes,
                )?;

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
                        )?;
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
                    &mut self.executor,
                    indexes,
                )?;

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
                    )?;
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

                self.executor.send(QueryMessage::other(query, arguments))?;

                // insert the new array elements
                for (idx, member) in array.iter().enumerate() {
                    let mut indexes = indexes.clone();
                    indexes.push(idx as i64);

                    update_members(
                        &[Member { name: "data".to_string(), ty: member.clone(), key: false }],
                        &mut self.executor,
                        &indexes,
                    )?;

                    let mut path_clone = path.clone();
                    path_clone.push("data".to_string());
                    self.build_set_entity_queries_recursive(
                        path_clone,
                        event_id,
                        (entity_id, is_event_message),
                        (member, is_store_update_member),
                        block_timestamp,
                        &indexes,
                    )?;
                }
            }
            _ => {}
        }

        Ok(())
    }

    fn build_delete_entity_queries_recursive(
        &mut self,
        path: Vec<String>,
        entity_id: &str,
        entity: &Ty,
    ) -> Result<()> {
        match entity {
            Ty::Struct(s) => {
                let table_id = path.join("$");
                let statement = format!("DELETE FROM [{table_id}] WHERE entity_id = ?");
                self.executor.send(QueryMessage::other(
                    statement,
                    vec![Argument::String(entity_id.to_string())],
                ))?;
                for member in s.children.iter() {
                    let mut path_clone = path.clone();
                    path_clone.push(member.name.clone());
                    self.build_delete_entity_queries_recursive(path_clone, entity_id, &member.ty)?;
                }
            }
            Ty::Enum(e) => {
                if e.options
                    .iter()
                    .all(|o| if let Ty::Tuple(t) = &o.ty { t.is_empty() } else { false })
                {
                    return Ok(());
                }

                let table_id = path.join("$");
                let statement = format!("DELETE FROM [{table_id}] WHERE entity_id = ?");
                self.executor.send(QueryMessage::other(
                    statement,
                    vec![Argument::String(entity_id.to_string())],
                ))?;

                for child in e.options.iter() {
                    if let Ty::Tuple(t) = &child.ty {
                        if t.is_empty() {
                            continue;
                        }
                    }

                    let mut path_clone = path.clone();
                    path_clone.push(child.name.clone());
                    self.build_delete_entity_queries_recursive(path_clone, entity_id, &child.ty)?;
                }
            }
            Ty::Array(array) => {
                let table_id = path.join("$");
                let statement = format!("DELETE FROM [{table_id}] WHERE entity_id = ?");
                self.executor.send(QueryMessage::other(
                    statement,
                    vec![Argument::String(entity_id.to_string())],
                ))?;

                for member in array.iter() {
                    let mut path_clone = path.clone();
                    path_clone.push("data".to_string());
                    self.build_delete_entity_queries_recursive(path_clone, entity_id, member)?;
                }
            }
            Ty::Tuple(t) => {
                let table_id = path.join("$");
                let statement = format!("DELETE FROM [{table_id}] WHERE entity_id = ?");
                self.executor.send(QueryMessage::other(
                    statement,
                    vec![Argument::String(entity_id.to_string())],
                ))?;

                for (idx, member) in t.iter().enumerate() {
                    let mut path_clone = path.clone();
                    path_clone.push(format!("_{}", idx));
                    self.build_delete_entity_queries_recursive(path_clone, entity_id, member)?;
                }
            }
            _ => {}
        }

        Ok(())
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
    ) -> Result<()> {
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

                    self.executor.send(QueryMessage::other(statement.to_string(), arguments))?;
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

                    self.executor.send(QueryMessage::other(statement.to_string(), arguments))?;
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

                self.executor.send(QueryMessage::other(statement.to_string(), arguments))?;
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

                    self.executor.send(QueryMessage::other(statement.to_string(), arguments))?;
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

        self.executor.send(QueryMessage::other(create_table_query, vec![]))?;

        for s in indices.iter() {
            self.executor.send(QueryMessage::other(s.to_string(), vec![]))?;
        }

        Ok(())
    }

    pub async fn execute(&self) -> Result<()> {
        let (execute, recv) = QueryMessage::execute_recv();
        self.executor.send(execute)?;
        recv.await?
    }
}
