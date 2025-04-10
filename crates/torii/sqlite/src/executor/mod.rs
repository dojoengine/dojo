use std::collections::{HashMap, HashSet};
use std::mem;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use cainome::cairo_serde::{ByteArray, CairoSerde};
use dojo_types::schema::{Struct, Ty};
use erc::{RegisterNftTokenMetadata, UpdateNftMetadata, UpdateNftMetadataQuery};
use sqlx::{FromRow, Pool, Sqlite, Transaction as SqlxTransaction};
use starknet::core::types::requests::CallRequest;
use starknet::core::types::{BlockId, BlockTag, Felt, FunctionCall};
use starknet::core::utils::{get_selector_from_name, parse_cairo_short_string};
use starknet::providers::{Provider, ProviderRequestData, ProviderResponseData};
use tokio::sync::broadcast::{Receiver, Sender};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::sync::{oneshot, Semaphore};
use tokio::task::JoinSet;
use tokio::time::Instant;
use tracing::{debug, error, info, warn};

use crate::constants::TOKENS_TABLE;
use crate::simple_broker::SimpleBroker;
use crate::types::{
    ContractCursor, ContractType, Entity as EntityUpdated, Event as EventEmitted,
    EventMessage as EventMessageUpdated, Model as ModelRegistered, OptimisticEntity,
    OptimisticEventMessage, ParsedCall, Token, TokenBalance, Transaction,
};
use crate::utils::{felt_and_u256_to_sql_string, felt_to_sql_string, felts_to_sql_string, I256};

pub mod erc;
pub use erc::{RegisterErc20TokenQuery, RegisterNftTokenQuery};

pub(crate) const LOG_TARGET: &str = "torii::sqlite::executor";

#[derive(Debug, Clone)]
pub enum Argument {
    Null,
    Int(i64),
    Bool(bool),
    String(String),
    FieldElement(Felt),
}

#[derive(Debug, Clone)]
pub enum BrokerMessage {
    SetHead(ContractCursor),
    ModelRegistered(ModelRegistered),
    EntityUpdated(EntityUpdated),
    EventMessageUpdated(EventMessageUpdated),
    EventEmitted(EventEmitted),
    TokenRegistered(Token),
    TokenBalanceUpdated(TokenBalance),
    Transaction(Transaction),
}

#[derive(Debug, Clone)]
pub struct DeleteEntityQuery {
    pub entity_id: String,
    pub model_id: String,
    pub event_id: String,
    pub block_timestamp: String,
    pub ty: Ty,
}

#[derive(Debug, Clone)]
pub struct ApplyBalanceDiffQuery {
    pub erc_cache: HashMap<(ContractType, String), I256>,
}

#[derive(Debug, Clone)]
pub struct SetHeadQuery {
    pub head: u64,
    pub last_block_timestamp: u64,
    pub txns_count: u64,
    pub contract_address: Felt,
}

#[derive(Debug, Clone)]
pub struct UpdateCursorsQuery {
    // contract => (last_txn, txn_count)
    pub cursor_map: HashMap<Felt, (Felt, u64)>,
    pub last_block_number: u64,
    pub last_block_timestamp: u64,
    pub last_pending_block_tx: Option<Felt>,
}

#[derive(Debug, Clone)]
pub struct EventMessageQuery {
    pub entity_id: String,
    pub model_id: String,
    pub keys_str: String,
    pub event_id: String,
    pub block_timestamp: String,
    pub is_historical: bool,
    pub ty: Ty,
}

#[derive(Debug, Clone)]
pub struct StoreTransactionQuery {
    pub contract_addresses: HashSet<Felt>,
    pub calls: Vec<ParsedCall>,
}

#[derive(Debug, Clone)]
pub struct EntityQuery {
    pub entity_id: String,
    pub model_id: String,
    pub keys_str: Option<String>,
    pub event_id: String,
    pub block_timestamp: String,
    pub is_historical: bool,
    pub ty: Ty,
}

#[derive(Debug, Clone)]
pub enum QueryType {
    StoreTransaction(StoreTransactionQuery),
    SetHead(SetHeadQuery),
    UpdateCursors(UpdateCursorsQuery),
    SetEntity(EntityQuery),
    DeleteEntity(DeleteEntityQuery),
    EventMessage(EventMessageQuery),
    ApplyBalanceDiff(ApplyBalanceDiffQuery),
    RegisterNftToken(RegisterNftTokenQuery),
    RegisterErc20Token(RegisterErc20TokenQuery),
    TokenTransfer,
    RegisterModel,
    StoreEvent,
    UpdateNftMetadata(UpdateNftMetadataQuery),
    Flush,
    Execute,
    Rollback,
    Other,
}

#[derive(Debug)]
pub struct Executor<'c, P: Provider + Sync + Send + 'static> {
    // Queries should use `transaction` instead of `pool`
    // This `pool` is only used to create a new `transaction`
    pool: Pool<Sqlite>,
    transaction: SqlxTransaction<'c, Sqlite>,
    publish_queue: Vec<BrokerMessage>,
    rx: UnboundedReceiver<QueryMessage>,
    shutdown_rx: Receiver<()>,
    // These tasks are spawned to fetch ERC721 token metadata from the chain
    // to not block the main loop
    register_tasks: JoinSet<Result<RegisterNftTokenMetadata>>,
    // Tasks for updating NFT metadata
    metadata_update_tasks: JoinSet<Result<UpdateNftMetadata>>,
    // Track which token IDs are currently being processed for metadata updates
    metadata_update_tokens: HashSet<String>,
    // Some queries depends on the metadata being registered, so we defer them
    // until the metadata is fetched
    deferred_query_messages: Vec<QueryMessage>,
    // It is used to make RPC calls to fetch token_uri data for erc721 contracts
    provider: Arc<P>,
    // Used to limit number of tasks that run in parallel to fetch metadata
    metadata_semaphore: Arc<Semaphore>,
}

#[derive(Debug)]
pub struct QueryMessage {
    pub statement: String,
    pub arguments: Vec<Argument>,
    pub query_type: QueryType,
    tx: Option<oneshot::Sender<Result<()>>>,
}

impl QueryMessage {
    pub fn new(statement: String, arguments: Vec<Argument>, query_type: QueryType) -> Self {
        Self { statement, arguments, query_type, tx: None }
    }

    pub fn new_recv(
        statement: String,
        arguments: Vec<Argument>,
        query_type: QueryType,
    ) -> (Self, oneshot::Receiver<Result<()>>) {
        let (tx, rx) = oneshot::channel();
        (Self { statement, arguments, query_type, tx: Some(tx) }, rx)
    }

    pub fn other(statement: String, arguments: Vec<Argument>) -> Self {
        Self { statement, arguments, query_type: QueryType::Other, tx: None }
    }

    pub fn other_recv(
        statement: String,
        arguments: Vec<Argument>,
    ) -> (Self, oneshot::Receiver<Result<()>>) {
        let (tx, rx) = oneshot::channel();
        (Self { statement, arguments, query_type: QueryType::Other, tx: Some(tx) }, rx)
    }

    pub fn execute() -> Self {
        Self {
            statement: "".to_string(),
            arguments: vec![],
            query_type: QueryType::Execute,
            tx: None,
        }
    }

    pub fn execute_recv() -> (Self, oneshot::Receiver<Result<()>>) {
        let (tx, rx) = oneshot::channel();
        (
            Self {
                statement: "".to_string(),
                arguments: vec![],
                query_type: QueryType::Execute,
                tx: Some(tx),
            },
            rx,
        )
    }

    pub fn flush_recv() -> (Self, oneshot::Receiver<Result<()>>) {
        let (tx, rx) = oneshot::channel();
        (
            Self {
                statement: "".to_string(),
                arguments: vec![],
                query_type: QueryType::Flush,
                tx: Some(tx),
            },
            rx,
        )
    }

    pub fn rollback_recv() -> (Self, oneshot::Receiver<Result<()>>) {
        let (tx, rx) = oneshot::channel();
        (
            Self {
                statement: "".to_string(),
                arguments: vec![],
                query_type: QueryType::Rollback,
                tx: Some(tx),
            },
            rx,
        )
    }
}

impl<'c, P: Provider + Sync + Send + 'static> Executor<'c, P> {
    pub async fn new(
        pool: Pool<Sqlite>,
        shutdown_tx: Sender<()>,
        provider: Arc<P>,
        max_metadata_tasks: usize,
    ) -> Result<(Self, UnboundedSender<QueryMessage>)> {
        let (tx, rx) = unbounded_channel();
        let transaction = pool.begin().await?;
        let publish_queue = Vec::new();
        let shutdown_rx = shutdown_tx.subscribe();
        let metadata_semaphore = Arc::new(Semaphore::new(max_metadata_tasks));

        Ok((
            Executor {
                pool,
                transaction,
                publish_queue,
                rx,
                shutdown_rx,
                register_tasks: JoinSet::new(),
                metadata_update_tasks: JoinSet::new(),
                metadata_update_tokens: HashSet::new(),
                deferred_query_messages: Vec::new(),
                provider,
                metadata_semaphore,
            },
            tx,
        ))
    }

    pub async fn run(&mut self) -> Result<()> {
        loop {
            tokio::select! {
                _ = self.shutdown_rx.recv() => {
                    debug!(target: LOG_TARGET, "Shutting down executor");
                    break Ok(());
                }
                Some(msg) = self.rx.recv() => {
                    let query_type = msg.query_type.clone();
                    match self.handle_query_message(msg).await {
                        Ok(()) => {},
                        Err(e) => {
                            error!(target: LOG_TARGET, r#type = ?query_type, error = %e, "Failed to execute query.");
                        }
                    }
                }
                Some(result) = self.register_tasks.join_next() => {
                    let result = result??;
                    self.handle_nft_token_metadata(result).await?;
                }
                Some(result) = self.metadata_update_tasks.join_next() => {
                    let result = result??;
                    self.handle_update_nft_metadata(result).await?;
                }
            }
        }
    }

    async fn handle_query_message(&mut self, query_message: QueryMessage) -> Result<()> {
        let tx = &mut self.transaction;

        let mut query = sqlx::query(&query_message.statement);

        for arg in &query_message.arguments {
            query = match arg {
                Argument::Null => query.bind(None::<String>),
                Argument::Int(integer) => query.bind(integer),
                Argument::Bool(bool) => query.bind(bool),
                Argument::String(string) => query.bind(string),
                Argument::FieldElement(felt) => query.bind(format!("{:#x}", felt)),
            }
        }

        match query_message.query_type {
            QueryType::SetHead(set_head) => {
                let previous_block_timestamp: u64 = sqlx::query_scalar::<_, i64>(
                    "SELECT last_block_timestamp FROM contracts WHERE id = ?",
                )
                .bind(format!("{:#x}", set_head.contract_address))
                .fetch_one(&mut **tx)
                .await?
                .try_into()
                .map_err(|_| anyhow::anyhow!("Last block timestamp doesn't fit in u64"))?;

                let tps: u64 = if set_head.last_block_timestamp - previous_block_timestamp != 0 {
                    set_head.txns_count / (set_head.last_block_timestamp - previous_block_timestamp)
                } else {
                    set_head.txns_count
                };

                query.execute(&mut **tx).await?;

                let row = sqlx::query("UPDATE contracts SET tps = ? WHERE id = ? RETURNING *")
                    .bind(tps as i64)
                    .bind(format!("{:#x}", set_head.contract_address))
                    .fetch_one(&mut **tx)
                    .await?;

                let contract = ContractCursor::from_row(&row)?;
                self.publish_queue.push(BrokerMessage::SetHead(contract));
            }
            QueryType::UpdateCursors(update_cursors) => {
                // Read all cursors from db
                let mut cursors: Vec<ContractCursor> =
                    sqlx::query_as("SELECT * FROM contracts").fetch_all(&mut **tx).await?;

                let new_head =
                    update_cursors.last_block_number.try_into().expect("doesn't fit in i64");
                let new_timestamp = update_cursors.last_block_timestamp;

                for cursor in &mut cursors {
                    if let Some(new_cursor) = update_cursors
                        .cursor_map
                        .get(&Felt::from_str(&cursor.contract_address).unwrap())
                    {
                        let cursor_timestamp: u64 =
                            cursor.last_block_timestamp.try_into().expect("doesn't fit in i64");

                        let num_transactions = new_cursor.1;

                        let new_tps = if new_timestamp - cursor_timestamp != 0 {
                            num_transactions / (new_timestamp - cursor_timestamp)
                        } else {
                            let current_time =
                                SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();

                            num_transactions / (current_time - cursor_timestamp)
                        };

                        cursor.last_pending_block_contract_tx =
                            if update_cursors.last_pending_block_tx.is_some() {
                                Some(felt_to_sql_string(&new_cursor.0))
                            } else {
                                None
                            };
                        cursor.tps = new_tps.try_into().expect("does't fit in i64");
                    } else {
                        cursor.tps = 0;
                    }
                    cursor.last_block_timestamp =
                        new_timestamp.try_into().expect("doesn't fit in i64");
                    cursor.head = new_head;
                    cursor.last_pending_block_tx =
                        update_cursors.last_pending_block_tx.map(|felt| felt_to_sql_string(&felt));

                    sqlx::query(
                        "UPDATE contracts SET head = ?, last_block_timestamp = ?, \
                         last_pending_block_tx = ?, last_pending_block_contract_tx = ? WHERE id = \
                         ?",
                    )
                    .bind(cursor.head)
                    .bind(cursor.last_block_timestamp)
                    .bind(&cursor.last_pending_block_tx)
                    .bind(&cursor.last_pending_block_contract_tx)
                    .bind(&cursor.contract_address)
                    .execute(&mut **tx)
                    .await?;

                    // Send appropriate ContractUpdated publish message
                    self.publish_queue.push(BrokerMessage::SetHead(cursor.clone()));
                }
            }
            QueryType::StoreTransaction(store_transaction) => {
                let row = query.fetch_one(&mut **tx).await?;
                let mut transaction = Transaction::from_row(&row)?;

                for contract_address in &store_transaction.contract_addresses {
                    sqlx::query(
                        "INSERT OR IGNORE INTO transaction_contract (transaction_hash, \
                         contract_address) VALUES (?, ?)",
                    )
                    .bind(&transaction.transaction_hash)
                    .bind(felt_to_sql_string(contract_address))
                    .execute(&mut **tx)
                    .await?;
                }

                // Store each call in the transaction_calls table
                for call in &store_transaction.calls {
                    sqlx::query(
                        "INSERT OR IGNORE INTO transaction_calls (transaction_hash, \
                         contract_address, entrypoint, calldata, call_type, caller_address) \
                         VALUES (?, ?, ?, ?, ?, ?)",
                    )
                    .bind(&transaction.transaction_hash)
                    .bind(felt_to_sql_string(&call.contract_address))
                    .bind(call.entrypoint.clone())
                    .bind(felts_to_sql_string(&call.calldata))
                    .bind(call.call_type.to_string())
                    .bind(felt_to_sql_string(&call.caller_address))
                    .execute(&mut **tx)
                    .await?;
                }

                transaction.contract_addresses = store_transaction.contract_addresses;
                transaction.calls = store_transaction.calls;

                self.publish_queue.push(BrokerMessage::Transaction(transaction));
            }
            QueryType::SetEntity(entity) => {
                let row = query.fetch_one(&mut **tx).await?;
                let mut entity_updated = EntityUpdated::from_row(&row)?;
                entity_updated.updated_model = Some(entity.ty.clone());
                entity_updated.deleted = false;

                if entity_updated.keys.is_empty() {
                    warn!(target: LOG_TARGET, "Entity has been updated without being set before. Keys are not known and non-updated values will be NULL.");
                }

                // Handle historical entities similar to historical event messages
                let mut entity_counter: i64 = sqlx::query_scalar::<_, i64>(
                    "SELECT historical_counter FROM entity_model WHERE entity_id = ? AND model_id \
                     = ?",
                )
                .bind(entity.entity_id.clone())
                .bind(entity.model_id.clone())
                .fetch_optional(&mut **tx)
                .await
                .map_or(0, |counter| counter.unwrap_or(0));

                if entity.is_historical {
                    entity_counter += 1;

                    let data = serde_json::to_string(&entity.ty.to_json_value()?)?;
                    if let Some(keys) = entity.keys_str {
                        sqlx::query(
                            "INSERT INTO entities_historical (id, keys, event_id, data, model_id, \
                             executed_at) VALUES (?, ?, ?, ?, ?, ?) RETURNING *",
                        )
                        .bind(entity.entity_id.clone())
                        .bind(keys)
                        .bind(entity.event_id.clone())
                        .bind(data)
                        .bind(entity.model_id.clone())
                        .bind(entity.block_timestamp.clone())
                        .fetch_one(&mut **tx)
                        .await?;
                    } else {
                        sqlx::query(
                            "INSERT INTO entities_historical (id, event_id, data, model_id, \
                             executed_at) VALUES (?, ?, ?, ?, ?) RETURNING *",
                        )
                        .bind(entity.entity_id.clone())
                        .bind(entity.event_id.clone())
                        .bind(data)
                        .bind(entity.model_id.clone())
                        .bind(entity.block_timestamp.clone())
                        .fetch_one(&mut **tx)
                        .await?;
                    }
                }

                sqlx::query(
                    "INSERT INTO entity_model (entity_id, model_id, historical_counter) VALUES \
                     (?, ?, ?) ON CONFLICT(entity_id, model_id) DO UPDATE SET \
                     historical_counter=EXCLUDED.historical_counter",
                )
                .bind(entity.entity_id.clone())
                .bind(entity.model_id.clone())
                .bind(entity_counter)
                .execute(&mut **tx)
                .await?;

                let optimistic_entity = unsafe {
                    std::mem::transmute::<EntityUpdated, OptimisticEntity>(entity_updated.clone())
                };
                SimpleBroker::publish(optimistic_entity);

                let broker_message = BrokerMessage::EntityUpdated(entity_updated);
                self.publish_queue.push(broker_message);
            }
            QueryType::DeleteEntity(entity) => {
                let delete_model = query.execute(&mut **tx).await?;
                if delete_model.rows_affected() == 0 {
                    return Ok(());
                }

                sqlx::query("DELETE FROM entity_model WHERE entity_id = ? AND model_id = ?")
                    .bind(entity.entity_id.clone())
                    .bind(entity.model_id)
                    .execute(&mut **tx)
                    .await?;

                let row = sqlx::query(
                    "UPDATE entities SET updated_at=CURRENT_TIMESTAMP, executed_at=?, event_id=? \
                     WHERE id = ? RETURNING *",
                )
                .bind(entity.block_timestamp)
                .bind(entity.event_id)
                .bind(entity.entity_id)
                .fetch_one(&mut **tx)
                .await?;
                let mut entity_updated = EntityUpdated::from_row(&row)?;
                entity_updated.updated_model =
                    Some(Ty::Struct(Struct { name: entity.ty.name(), children: vec![] }));

                let count = sqlx::query_scalar::<_, i64>(
                    "SELECT count(*) FROM entity_model WHERE entity_id = ?",
                )
                .bind(entity_updated.id.clone())
                .fetch_one(&mut **tx)
                .await?;

                // Delete entity if all of its models are deleted
                if count == 0 {
                    sqlx::query("DELETE FROM entities WHERE id = ?")
                        .bind(entity_updated.id.clone())
                        .execute(&mut **tx)
                        .await?;
                    entity_updated.deleted = true;
                }

                SimpleBroker::publish(unsafe {
                    std::mem::transmute::<EntityUpdated, OptimisticEntity>(entity_updated.clone())
                });
                self.publish_queue.push(BrokerMessage::EntityUpdated(entity_updated));
            }
            QueryType::RegisterModel => {
                let row = query.fetch_one(&mut **tx).await?;
                let model_registered = ModelRegistered::from_row(&row)?;
                self.publish_queue.push(BrokerMessage::ModelRegistered(model_registered));
            }
            QueryType::EventMessage(em_query) => {
                // Must be executed first since other tables have foreign keys on event_messages.id.
                let event_messages_row = query.fetch_one(&mut **tx).await?;
                let mut event_counter: i64 = sqlx::query_scalar::<_, i64>(
                    "SELECT historical_counter FROM event_model WHERE entity_id = ? AND model_id \
                     = ?",
                )
                .bind(em_query.entity_id.clone())
                .bind(em_query.model_id.clone())
                .fetch_optional(&mut **tx)
                .await
                .map_or(0, |counter| counter.unwrap_or(0));

                if em_query.is_historical {
                    event_counter += 1;

                    let data = serde_json::to_string(&em_query.ty.to_json_value()?)?;
                    sqlx::query(
                        "INSERT INTO event_messages_historical (id, keys, event_id, data, \
                         model_id, executed_at) VALUES (?, ?, ?, ?, ?, ?) RETURNING *",
                    )
                    .bind(em_query.entity_id.clone())
                    .bind(em_query.keys_str.clone())
                    .bind(em_query.event_id.clone())
                    .bind(data)
                    .bind(em_query.model_id.clone())
                    .bind(em_query.block_timestamp.clone())
                    .fetch_one(&mut **tx)
                    .await?;
                }

                sqlx::query(
                    "INSERT INTO event_model (entity_id, model_id, historical_counter) VALUES (?, \
                     ?, ?) ON CONFLICT(entity_id, model_id) DO UPDATE SET \
                     historical_counter=EXCLUDED.historical_counter",
                )
                .bind(em_query.entity_id.clone())
                .bind(em_query.model_id.clone())
                .bind(event_counter)
                .execute(&mut **tx)
                .await?;

                let mut event_message = EventMessageUpdated::from_row(&event_messages_row)?;
                event_message.updated_model = Some(em_query.ty);

                SimpleBroker::publish(unsafe {
                    std::mem::transmute::<EventMessageUpdated, OptimisticEventMessage>(
                        event_message.clone(),
                    )
                });
                self.publish_queue.push(BrokerMessage::EventMessageUpdated(event_message));
            }
            QueryType::StoreEvent => {
                let row = query.fetch_one(&mut **tx).await?;
                let event = EventEmitted::from_row(&row)?;
                self.publish_queue.push(BrokerMessage::EventEmitted(event));
            }
            QueryType::ApplyBalanceDiff(apply_balance_diff) => {
                debug!(target: LOG_TARGET, "Applying balance diff.");
                let instant = Instant::now();
                self.apply_balance_diff(apply_balance_diff, self.provider.clone()).await?;
                debug!(target: LOG_TARGET, duration = ?instant.elapsed(), "Applied balance diff.");
            }
            QueryType::RegisterNftToken(register_nft_token) => {
                let metadata_semaphore = self.metadata_semaphore.clone();
                let provider = self.provider.clone();

                // Check if we already have the metadata for this contract
                let res = sqlx::query_as::<_, (String, String)>(&format!(
                    "SELECT name, symbol FROM {TOKENS_TABLE} WHERE contract_address = ? LIMIT 1"
                ))
                .bind(felt_to_sql_string(&register_nft_token.contract_address))
                .fetch_one(&mut **tx)
                .await;

                // If we find a token already registered for this contract_address we dont need to
                // refetch the data since its same for all tokens of this contract
                let (name, symbol) = match res {
                    Ok((name, symbol)) => {
                        debug!(
                            target: LOG_TARGET,
                            contract_address = %felt_to_sql_string(&register_nft_token.contract_address),
                            "Token already registered for contract_address, so reusing fetched data",
                        );
                        (name, symbol)
                    }
                    Err(_) => {
                        // Prepare batch requests for name and symbol
                        let block_id = BlockId::Tag(BlockTag::Pending);
                        let requests = vec![
                            ProviderRequestData::Call(CallRequest {
                                request: FunctionCall {
                                    contract_address: register_nft_token.contract_address,
                                    entry_point_selector: get_selector_from_name("name").unwrap(),
                                    calldata: vec![],
                                },
                                block_id,
                            }),
                            ProviderRequestData::Call(CallRequest {
                                request: FunctionCall {
                                    contract_address: register_nft_token.contract_address,
                                    entry_point_selector: get_selector_from_name("symbol").unwrap(),
                                    calldata: vec![],
                                },
                                block_id,
                            }),
                        ];

                        let results = provider.batch_requests(requests).await;
                        match results {
                            Ok(results) => {
                                // Parse name
                                let name = match &results[0] {
                                    ProviderResponseData::Call(name) if name.len() == 1 => {
                                        parse_cairo_short_string(&name[0])?
                                    }
                                    ProviderResponseData::Call(name) => {
                                        ByteArray::cairo_deserialize(name, 0)?.to_string()?
                                    }
                                    _ => String::new(),
                                };

                                // Parse symbol
                                let symbol = match &results[1] {
                                    ProviderResponseData::Call(symbol) if symbol.len() == 1 => {
                                        parse_cairo_short_string(&symbol[0])?
                                    }
                                    ProviderResponseData::Call(symbol) => {
                                        ByteArray::cairo_deserialize(symbol, 0)?.to_string()?
                                    }
                                    _ => String::new(),
                                };

                                (name, symbol)
                            }
                            _ => (String::new(), String::new()),
                        }
                    }
                };

                self.register_tasks.spawn(async move {
                    let permit = metadata_semaphore.acquire().await.unwrap();

                    let metadata = Self::fetch_token_metadata(
                        register_nft_token.contract_address,
                        register_nft_token.token_id,
                        provider,
                    )
                    .await?;

                    drop(permit);
                    Ok(RegisterNftTokenMetadata {
                        query: register_nft_token,
                        name,
                        symbol,
                        metadata,
                    })
                });
            }
            QueryType::RegisterErc20Token(register_erc20_token) => {
                let query = sqlx::query_as::<_, Token>(
                    "INSERT INTO tokens (id, contract_address, name, symbol, decimals) VALUES (?, \
                     ?, ?, ?, ?) RETURNING *",
                )
                .bind(&register_erc20_token.token_id)
                .bind(felt_to_sql_string(&register_erc20_token.contract_address))
                .bind(&register_erc20_token.name)
                .bind(&register_erc20_token.symbol)
                .bind(register_erc20_token.decimals);

                let token = query.fetch_one(&mut **tx).await?;
                info!(target: LOG_TARGET, name = %register_erc20_token.name, symbol = %register_erc20_token.symbol, contract_address = %token.contract_address, "Registered ERC20 token.");

                self.publish_queue.push(BrokerMessage::TokenRegistered(token));
            }
            QueryType::Flush => {
                debug!(target: LOG_TARGET, "Flushing query.");
                let instant = Instant::now();
                let res = self.execute(false).await;
                debug!(target: LOG_TARGET, duration = ?instant.elapsed(), "Flushed query.");

                if let Some(sender) = query_message.tx {
                    sender
                        .send(res)
                        .map_err(|_| anyhow::anyhow!("Failed to send execute result"))?;
                } else {
                    res?;
                }
            }
            QueryType::Execute => {
                debug!(target: LOG_TARGET, "Executing query.");
                let instant = Instant::now();
                let res = self.execute(true).await;
                debug!(target: LOG_TARGET, duration = ?instant.elapsed(), "Executed query.");

                if let Some(sender) = query_message.tx {
                    sender
                        .send(res)
                        .map_err(|_| anyhow::anyhow!("Failed to send execute result"))?;
                } else {
                    res?;
                }
            }
            QueryType::TokenTransfer => {
                // defer executing these queries since they depend on TokenRegister queries
                self.deferred_query_messages.push(query_message);
            }
            QueryType::Rollback => {
                debug!(target: LOG_TARGET, "Rolling back the transaction.");
                // rollback's the current transaction and starts a new one
                let res = self.rollback().await;
                debug!(target: LOG_TARGET, "Rolled back the transaction.");

                if let Some(sender) = query_message.tx {
                    sender
                        .send(res)
                        .map_err(|_| anyhow::anyhow!("Failed to send rollback result"))?;
                } else {
                    res?;
                }
            }
            QueryType::UpdateNftMetadata(update_metadata) => {
                debug!(target: LOG_TARGET, "Updating NFT metadata.");
                let instant = Instant::now();

                let metadata_semaphore = self.metadata_semaphore.clone();
                let provider = self.provider.clone();

                let token_id = felt_and_u256_to_sql_string(
                    &update_metadata.contract_address,
                    &update_metadata.token_id,
                );
                if self.metadata_update_tokens.contains(&token_id) {
                    return Ok(());
                }

                self.metadata_update_tasks.spawn(async move {
                    let permit = metadata_semaphore.acquire().await.unwrap();

                    let metadata = Self::fetch_token_metadata(
                        update_metadata.contract_address,
                        update_metadata.token_id,
                        provider,
                    )
                    .await?;

                    drop(permit);
                    Ok(UpdateNftMetadata { token_id, metadata })
                });
                debug!(target: LOG_TARGET, duration = ?instant.elapsed(), "Updated NFT metadata.");
            }
            QueryType::Other => {
                query.execute(&mut **tx).await?;
            }
        }

        Ok(())
    }

    async fn execute(&mut self, new_transaction: bool) -> Result<()> {
        if new_transaction {
            let transaction = mem::replace(&mut self.transaction, self.pool.begin().await?);
            transaction.commit().await?;

            for message in self.publish_queue.drain(..) {
                send_broker_message(message);
            }
        }

        while let Some(result) = self.register_tasks.join_next().await {
            let result = result??;
            self.handle_nft_token_metadata(result).await?;
        }

        while let Some(result) = self.metadata_update_tasks.join_next().await {
            let result = result??;
            self.handle_update_nft_metadata(result).await?;
        }

        let mut deferred_query_messages = mem::take(&mut self.deferred_query_messages);

        for query_message in deferred_query_messages.drain(..) {
            let mut query = sqlx::query(&query_message.statement);
            for arg in &query_message.arguments {
                query = match arg {
                    Argument::Null => query.bind(None::<String>),
                    Argument::Int(integer) => query.bind(integer),
                    Argument::Bool(bool) => query.bind(bool),
                    Argument::String(string) => query.bind(string),
                    Argument::FieldElement(felt) => query.bind(format!("{:#x}", felt)),
                };
            }

            query.execute(&mut *self.transaction).await?;
        }

        Ok(())
    }

    async fn rollback(&mut self) -> Result<()> {
        let transaction = mem::replace(&mut self.transaction, self.pool.begin().await?);
        transaction.rollback().await?;

        // NOTE: clear doesn't reset the capacity
        self.publish_queue.clear();
        self.deferred_query_messages.clear();
        Ok(())
    }
}

fn send_broker_message(message: BrokerMessage) {
    match message {
        BrokerMessage::SetHead(update) => SimpleBroker::publish(update),
        BrokerMessage::ModelRegistered(model) => SimpleBroker::publish(model),
        BrokerMessage::EntityUpdated(entity) => SimpleBroker::publish(entity),
        BrokerMessage::EventMessageUpdated(event) => SimpleBroker::publish(event),
        BrokerMessage::EventEmitted(event) => SimpleBroker::publish(event),
        BrokerMessage::TokenRegistered(token) => SimpleBroker::publish(token),
        BrokerMessage::TokenBalanceUpdated(token_balance) => SimpleBroker::publish(token_balance),
        BrokerMessage::Transaction(transaction) => SimpleBroker::publish(transaction),
    }
}
