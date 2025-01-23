use std::collections::HashMap;
use std::convert::TryInto;
use std::str::FromStr;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use dojo_types::naming::get_tag;
use dojo_types::schema::{Struct, Ty};
use dojo_world::config::WorldMetadata;
use dojo_world::contracts::abigen::model::Layout;
use dojo_world::contracts::naming::compute_selector_from_names;
use sqlx::{Pool, Sqlite};
use starknet::core::types::{Event, Felt, InvokeTransaction, Transaction};
use starknet_crypto::poseidon_hash_many;
use tokio::sync::mpsc::UnboundedSender;
use utils::felts_to_sql_string;

use crate::constants::SQL_FELT_DELIMITER;
use crate::executor::{
    Argument, DeleteEntityQuery, EventMessageQuery, QueryMessage, QueryType, ResetCursorsQuery,
    SetHeadQuery, UpdateCursorsQuery,
};
use crate::types::Contract;
use crate::utils::utc_dt_string_from_timestamp;

type IsEventMessage = bool;
type IsStoreUpdate = bool;

pub mod cache;
pub mod constants;
pub mod erc;
pub mod error;
pub mod executor;
pub mod model;
pub mod simple_broker;
pub mod types;
pub mod utils;

use cache::{LocalCache, Model, ModelCache};

#[derive(Debug, Clone)]
pub struct Sql {
    pub pool: Pool<Sqlite>,
    pub executor: UnboundedSender<QueryMessage>,
    model_cache: Arc<ModelCache>,
    local_cache: Arc<LocalCache>,
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
        contracts: &[Contract],
        model_cache: Arc<ModelCache>,
    ) -> Result<Self> {
        for contract in contracts {
            executor.send(QueryMessage::other(
                "INSERT OR IGNORE INTO contracts (id, contract_address, contract_type) VALUES (?, \
                 ?, ?)"
                    .to_string(),
                vec![
                    Argument::FieldElement(contract.address),
                    Argument::FieldElement(contract.address),
                    Argument::String(contract.r#type.to_string()),
                ],
            ))?;
        }

        let local_cache = LocalCache::new(pool.clone()).await;
        let db =
            Self { pool: pool.clone(), executor, model_cache, local_cache: Arc::new(local_cache) };

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

    pub async fn cursors(&self) -> Result<Cursors> {
        let cursors = sqlx::query_as::<_, (String, String)>(
            "SELECT contract_address, last_pending_block_contract_tx FROM contracts WHERE \
             last_pending_block_contract_tx IS NOT NULL",
        )
        .fetch_all(&self.pool)
        .await?;

        let (head, last_pending_block_tx) = sqlx::query_as::<_, (Option<i64>, Option<String>)>(
            "SELECT head, last_pending_block_tx FROM contracts WHERE 1=1",
        )
        .fetch_one(&self.pool)
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
        model: &Ty,
        layout: Layout,
        class_hash: Felt,
        contract_address: Felt,
        packed_size: u32,
        unpacked_size: u32,
        block_timestamp: u64,
        upgrade_diff: Option<&Ty>,
    ) -> Result<()> {
        let selector = compute_selector_from_names(namespace, &model.name());
        let namespaced_name = get_tag(namespace, &model.name());
        let namespaced_schema = Ty::Struct(Struct {
            name: namespaced_name.clone(),
            children: model.as_struct().unwrap().children.clone(),
        });

        let insert_models =
            "INSERT INTO models (id, namespace, name, class_hash, contract_address, layout, \
             schema, packed_size, unpacked_size, executed_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, \
             ?) ON CONFLICT(id) DO UPDATE SET contract_address=EXCLUDED.contract_address, \
             class_hash=EXCLUDED.class_hash, layout=EXCLUDED.layout, schema=EXCLUDED.schema, \
             packed_size=EXCLUDED.packed_size, unpacked_size=EXCLUDED.unpacked_size, \
             executed_at=EXCLUDED.executed_at RETURNING *";
        let arguments = vec![
            Argument::String(format!("{:#x}", selector)),
            Argument::String(namespace.to_string()),
            Argument::String(model.name().to_string()),
            Argument::String(format!("{class_hash:#x}")),
            Argument::String(format!("{contract_address:#x}")),
            Argument::String(serde_json::to_string(&layout)?),
            Argument::String(serde_json::to_string(&namespaced_schema)?),
            Argument::Int(packed_size as i64),
            Argument::Int(unpacked_size as i64),
            Argument::String(utc_dt_string_from_timestamp(block_timestamp)),
        ];
        self.executor.send(QueryMessage::new(
            insert_models.to_string(),
            arguments,
            QueryType::RegisterModel,
        ))?;

        self.build_model_query(vec![namespaced_name.clone()], model, upgrade_diff)?;

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
                    schema: namespaced_schema,
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

        self.set_entity_model(
            &namespaced_name,
            event_id,
            (&entity_id, false),
            (&entity, keys_str.is_none()),
            block_timestamp,
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

        self.set_entity_model(
            &namespaced_name,
            event_id,
            (&entity_id, true),
            (&entity, false),
            block_timestamp,
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
        let model_table = entity.name();

        self.executor.send(QueryMessage::new(
            format!(
                "DELETE FROM [{model_table}] WHERE internal_id = ?; DELETE FROM entity_model \
                 WHERE entity_id = ? AND model_id = ?"
            )
            .to_string(),
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
                Transaction::Invoke(InvokeTransaction::V3(invoke_v3_transaction)) => (
                    Argument::FieldElement(invoke_v3_transaction.transaction_hash),
                    Argument::FieldElement(invoke_v3_transaction.sender_address),
                    Argument::String(felts_to_sql_string(&invoke_v3_transaction.calldata)),
                    Argument::FieldElement(Felt::ZERO), // has no max_fee
                    Argument::String(felts_to_sql_string(&invoke_v3_transaction.signature)),
                    Argument::FieldElement(invoke_v3_transaction.nonce),
                ),
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

    fn set_entity_model(
        &mut self,
        model_name: &str,
        event_id: &str,
        entity_id: (&str, IsEventMessage),
        entity: (&Ty, IsStoreUpdate),
        block_timestamp: u64,
    ) -> Result<()> {
        let (entity_id, is_event_message) = entity_id;
        let (entity, is_store_update) = entity;

        let mut columns = vec![
            "internal_id".to_string(),
            "internal_event_id".to_string(),
            "internal_executed_at".to_string(),
            "internal_updated_at".to_string(),
            if is_event_message {
                "internal_event_message_id".to_string()
            } else {
                "internal_entity_id".to_string()
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

        fn collect_members(
            prefix: &str,
            ty: &Ty,
            columns: &mut Vec<String>,
            arguments: &mut Vec<Argument>,
        ) -> Result<()> {
            match ty {
                Ty::Struct(s) => {
                    for member in &s.children {
                        let column_name = if prefix.is_empty() {
                            member.name.clone()
                        } else {
                            format!("{}.{}", prefix, member.name)
                        };
                        collect_members(&column_name, &member.ty, columns, arguments)?;
                    }
                }
                Ty::Enum(e) => {
                    columns.push(format!("\"{}\"", prefix));
                    arguments.push(Argument::String(e.to_sql_value()));

                    if let Some(option_idx) = e.option {
                        let option = &e.options[option_idx as usize];
                        if let Ty::Tuple(t) = &option.ty {
                            if t.is_empty() {
                                return Ok(());
                            }
                        }
                        let variant_path = format!("{}.{}", prefix, option.name);
                        collect_members(&variant_path, &option.ty, columns, arguments)?;
                    }
                }
                Ty::Tuple(t) => {
                    for (idx, member) in t.iter().enumerate() {
                        let column_name = if prefix.is_empty() {
                            format!("{}", idx)
                        } else {
                            format!("{}.{}", prefix, idx)
                        };
                        collect_members(&column_name, member, columns, arguments)?;
                    }
                }
                Ty::Array(array) => {
                    columns.push(format!("\"{}\"", prefix));
                    let values =
                        array.iter().map(|v| v.to_json_value()).collect::<Result<Vec<_>, _>>()?;
                    arguments.push(Argument::String(serde_json::to_string(&values)?));
                }
                Ty::Primitive(ty) => {
                    columns.push(format!("\"{}\"", prefix));
                    arguments.push(Argument::String(ty.to_sql_value()));
                }
                Ty::ByteArray(b) => {
                    columns.push(format!("\"{}\"", prefix));
                    arguments.push(Argument::String(b.clone()));
                }
            }
            Ok(())
        }

        // Collect all columns and arguments recursively
        collect_members("", entity, &mut columns, &mut arguments)?;

        // Build the final query
        let placeholders: Vec<&str> = arguments.iter().map(|_| "?").collect();
        let statement = if is_store_update {
            arguments.push(Argument::String(if is_event_message {
                "event:".to_string() + entity_id
            } else {
                entity_id.to_string()
            }));

            format!(
                "UPDATE [{}] SET {} WHERE internal_id = ?",
                model_name,
                columns
                    .iter()
                    .zip(placeholders.iter())
                    .map(|(column, placeholder)| format!("{} = {}", column, placeholder))
                    .collect::<Vec<String>>()
                    .join(", ")
            )
        } else {
            format!(
                "INSERT OR REPLACE INTO [{}] ({}) VALUES ({})",
                model_name,
                columns.join(","),
                placeholders.join(",")
            )
        };

        // Execute the single query
        self.executor.send(QueryMessage::other(statement, arguments))?;

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn build_model_query(
        &mut self,
        path: Vec<String>,
        model: &Ty,
        upgrade_diff: Option<&Ty>,
    ) -> Result<()> {
        let table_id = path[0].clone(); // Use only the root path component
        let mut columns = Vec::new();
        let mut indices = Vec::new();
        let mut alter_table_queries = Vec::new();

        // Start building the create table query with internal columns
        let mut create_table_query = format!(
            "CREATE TABLE IF NOT EXISTS [{table_id}] (internal_id TEXT NOT NULL PRIMARY KEY, \
             internal_event_id TEXT NOT NULL, internal_entity_id TEXT, internal_event_message_id \
             TEXT, "
        );

        indices.push(format!(
            "CREATE INDEX IF NOT EXISTS [idx_{table_id}_internal_entity_id] ON [{table_id}] \
             ([internal_entity_id]);"
        ));
        indices.push(format!(
            "CREATE INDEX IF NOT EXISTS [idx_{table_id}_internal_event_message_id] ON \
             [{table_id}] ([internal_event_message_id]);"
        ));

        // Recursively add columns for all nested type
        add_columns_recursive(
            &path,
            model,
            &mut columns,
            &mut alter_table_queries,
            &mut indices,
            &table_id,
            upgrade_diff,
        )?;

        // Add all columns to the create table query
        for column in columns {
            create_table_query.push_str(&format!("{}, ", column));
        }

        // Add internal timestamps
        create_table_query.push_str("internal_executed_at DATETIME NOT NULL, ");
        create_table_query
            .push_str("internal_created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP, ");
        create_table_query
            .push_str("internal_updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP, ");

        // Add foreign key constraints
        create_table_query.push_str("FOREIGN KEY (internal_entity_id) REFERENCES entities(id), ");
        create_table_query
            .push_str("FOREIGN KEY (internal_event_message_id) REFERENCES event_messages(id));");

        // Execute the queries
        if upgrade_diff.is_some() {
            for alter_query in alter_table_queries {
                self.executor.send(QueryMessage::other(alter_query, vec![]))?;
            }
        } else {
            self.executor.send(QueryMessage::other(create_table_query, vec![]))?;
        }

        // Create indices
        for index_query in indices {
            self.executor.send(QueryMessage::other(index_query, vec![]))?;
        }

        Ok(())
    }

    pub async fn execute(&self) -> Result<()> {
        let (execute, recv) = QueryMessage::execute_recv();
        self.executor.send(execute)?;
        recv.await?
    }

    pub async fn flush(&self) -> Result<()> {
        let (flush, recv) = QueryMessage::flush_recv();
        self.executor.send(flush)?;
        recv.await?
    }

    pub async fn rollback(&self) -> Result<()> {
        let (rollback, recv) = QueryMessage::rollback_recv();
        self.executor.send(rollback)?;
        recv.await?
    }
}

fn add_columns_recursive(
    path: &[String],
    ty: &Ty,
    columns: &mut Vec<String>,
    alter_table_queries: &mut Vec<String>,
    indices: &mut Vec<String>,
    table_id: &str,
    upgrade_diff: Option<&Ty>,
) -> Result<()> {
    let column_prefix = if path.len() > 1 { path[1..].join(".") } else { String::new() };

    let mut add_column = |name: &str, sql_type: &str| {
        if upgrade_diff.is_some() {
            alter_table_queries
                .push(format!("ALTER TABLE [{table_id}] ADD COLUMN [{name}] {sql_type}"));
        } else {
            columns.push(format!("[{name}] {sql_type}"));
        }
        indices.push(format!(
            "CREATE INDEX IF NOT EXISTS [idx_{table_id}_{name}] ON [{table_id}] ([{name}]);"
        ));
    };

    match ty {
        Ty::Struct(s) => {
            for member in &s.children {
                if let Some(upgrade_diff) = upgrade_diff {
                    if !upgrade_diff
                        .as_struct()
                        .unwrap()
                        .children
                        .iter()
                        .any(|m| m.name == member.name)
                    {
                        continue;
                    }
                }

                let mut new_path = path.to_vec();
                new_path.push(member.name.clone());

                add_columns_recursive(
                    &new_path,
                    &member.ty,
                    columns,
                    alter_table_queries,
                    indices,
                    table_id,
                    None,
                )?;
            }
        }
        Ty::Tuple(tuple) => {
            for (idx, member) in tuple.iter().enumerate() {
                let mut new_path = path.to_vec();
                new_path.push(idx.to_string());

                add_columns_recursive(
                    &new_path,
                    member,
                    columns,
                    alter_table_queries,
                    indices,
                    table_id,
                    None,
                )?;
            }
        }
        Ty::Array(_) => {
            let column_name =
                if column_prefix.is_empty() { "value".to_string() } else { column_prefix };

            add_column(&column_name, "TEXT");
        }
        Ty::Enum(e) => {
            // The variant of the enum
            let column_name =
                if column_prefix.is_empty() { "option".to_string() } else { column_prefix };

            let all_options =
                e.options.iter().map(|c| format!("'{}'", c.name)).collect::<Vec<_>>().join(", ");

            let sql_type = format!("TEXT CHECK([{column_name}] IN ({all_options}))");
            add_column(&column_name, &sql_type);

            for child in &e.options {
                if let Ty::Tuple(tuple) = &child.ty {
                    if tuple.is_empty() {
                        continue;
                    }
                }

                let mut new_path = path.to_vec();
                new_path.push(child.name.clone());

                add_columns_recursive(
                    &new_path,
                    &child.ty,
                    columns,
                    alter_table_queries,
                    indices,
                    table_id,
                    None,
                )?;
            }
        }
        Ty::ByteArray(_) => {
            let column_name =
                if column_prefix.is_empty() { "value".to_string() } else { column_prefix };

            add_column(&column_name, "TEXT");
        }
        Ty::Primitive(p) => {
            let column_name =
                if column_prefix.is_empty() { "value".to_string() } else { column_prefix };

            add_column(&column_name, p.to_sql_type().as_ref());
        }
    }

    Ok(())
}
