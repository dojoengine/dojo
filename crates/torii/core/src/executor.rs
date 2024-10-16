use std::collections::HashMap;
use std::mem;
use std::str::FromStr;
use std::sync::Arc;

use anyhow::{Context, Result};
use cainome::cairo_serde::{ByteArray, CairoSerde};
use camino::Utf8PathBuf;
use data_url::mime::Mime;
use data_url::DataUrl;
use dojo_types::schema::{Struct, Ty};
use reqwest::Client;
use sqlx::{FromRow, Pool, Sqlite, Transaction};
use starknet::core::types::{BlockId, BlockTag, Felt, FunctionCall, U256};
use starknet::core::utils::{get_selector_from_name, parse_cairo_short_string};
use starknet::providers::Provider;
use tokio::sync::broadcast::{Receiver, Sender};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::sync::{oneshot, Semaphore};
use tokio::task::JoinSet;
use tokio::time::Instant;
use tracing::{debug, error, trace};

use crate::simple_broker::SimpleBroker;
use crate::sql::utils::{felt_to_sql_string, sql_string_to_u256, u256_to_sql_string, I256};
use crate::sql::FELT_DELIMITER;
use crate::types::{
    ContractCursor, ContractType, Entity as EntityUpdated, Event as EventEmitted,
    EventMessage as EventMessageUpdated, Model as ModelRegistered, OptimisticEntity,
    OptimisticEventMessage,
};
use crate::utils::{fetch_content_from_ipfs, MAX_RETRY};

pub(crate) const LOG_TARGET: &str = "torii_core::executor";

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
}

#[derive(Debug, Clone)]
pub struct DeleteEntityQuery {
    pub entity_id: String,
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
pub struct ResetCursorsQuery {
    // contract => (last_txn, txn_count)
    pub cursor_map: HashMap<Felt, (Felt, u64)>,
    pub last_block_timestamp: u64,
    pub last_block_number: u64,
}

#[derive(Debug, Clone)]
pub struct UpdateCursorsQuery {
    // contract => (last_txn, txn_count)
    pub cursor_map: HashMap<Felt, (Felt, u64)>,
    pub last_block_number: u64,
    pub last_pending_block_tx: Option<Felt>,
    pub pending_block_timestamp: u64,
}

#[derive(Debug, Clone)]
pub struct RegisterErc721TokenQuery {
    pub token_id: String,
    pub contract_address: Felt,
    pub actual_token_id: U256,
}

#[derive(Debug, Clone)]
pub struct RegisterErc721TokenMetadata {
    pub query: RegisterErc721TokenQuery,
    pub name: String,
    pub symbol: String,
    pub metadata: String,
}

#[derive(Debug, Clone)]
pub struct RegisterErc20TokenQuery {
    pub token_id: String,
    pub contract_address: Felt,
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
}

#[derive(Debug, Clone)]
pub enum QueryType {
    SetHead(SetHeadQuery),
    ResetCursors(ResetCursorsQuery),
    UpdateCursors(UpdateCursorsQuery),
    SetEntity(Ty),
    DeleteEntity(DeleteEntityQuery),
    EventMessage(Ty),
    ApplyBalanceDiff(ApplyBalanceDiffQuery),
    RegisterErc721Token(RegisterErc721TokenQuery),
    RegisterErc20Token(RegisterErc20TokenQuery),
    TokenTransfer,
    RegisterModel,
    StoreEvent,
    // similar to execute but doesn't create a new transaction
    Flush,
    Execute,
    Other,
}

#[derive(Debug)]
pub struct Executor<'c, P: Provider + Sync + Send + 'static> {
    // Queries should use `transaction` instead of `pool`
    // This `pool` is only used to create a new `transaction`
    pool: Pool<Sqlite>,
    transaction: Transaction<'c, Sqlite>,
    publish_queue: Vec<BrokerMessage>,
    artifacts_path: Utf8PathBuf,
    rx: UnboundedReceiver<QueryMessage>,
    shutdown_rx: Receiver<()>,
    ongoing_futures: JoinSet<Result<RegisterErc721TokenMetadata>>,
    deferred_query_messages: Vec<QueryMessage>,
    provider: Arc<P>,
    semaphore: Arc<Semaphore>,
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
}

impl<'c, P: Provider + Sync + Send + 'static> Executor<'c, P> {
    pub async fn new(
        pool: Pool<Sqlite>,
        shutdown_tx: Sender<()>,
        artifacts_path: &Utf8PathBuf,
        provider: Arc<P>,
        max_concurrent_tasks: usize,
    ) -> Result<(Self, UnboundedSender<QueryMessage>)> {
        let (tx, rx) = unbounded_channel();
        let transaction = pool.begin().await?;
        let publish_queue = Vec::new();
        let shutdown_rx = shutdown_tx.subscribe();
        let semaphore = Arc::new(Semaphore::new(max_concurrent_tasks));

        Ok((
            Executor {
                pool,
                transaction,
                publish_queue,
                rx,
                shutdown_rx,
                artifacts_path: artifacts_path.clone(),
                ongoing_futures: JoinSet::new(),
                deferred_query_messages: Vec::new(),
                provider,
                semaphore,
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
                Some(result) = self.ongoing_futures.join_next() => {
                    let result = result??;
                    self.handle_erc721_token_metadata(result).await?;
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

                query.execute(&mut **tx).await.with_context(|| {
                    format!(
                        "Failed to execute query: {:?}, args: {:?}",
                        query_message.statement, query_message.arguments
                    )
                })?;

                let row = sqlx::query("UPDATE contracts SET tps = ? WHERE id = ? RETURNING *")
                    .bind(tps as i64)
                    .bind(format!("{:#x}", set_head.contract_address))
                    .fetch_one(&mut **tx)
                    .await?;

                let contract = ContractCursor::from_row(&row)?;
                self.publish_queue.push(BrokerMessage::SetHead(contract));
            }
            QueryType::ResetCursors(reset_heads) => {
                // Read all cursors from db
                let mut cursors: Vec<ContractCursor> =
                    sqlx::query_as("SELECT * FROM contracts").fetch_all(&mut **tx).await?;

                let new_head =
                    reset_heads.last_block_number.try_into().expect("doesn't fit in i64");
                let new_timestamp = reset_heads.last_block_timestamp;

                for cursor in &mut cursors {
                    if let Some(new_cursor) = reset_heads
                        .cursor_map
                        .get(&Felt::from_str(&cursor.contract_address).unwrap())
                    {
                        let cursor_timestamp: u64 =
                            cursor.last_block_timestamp.try_into().expect("doesn't fit in i64");

                        let new_tps = if new_timestamp - cursor_timestamp != 0 {
                            new_cursor.1 / (new_timestamp - cursor_timestamp)
                        } else {
                            new_cursor.1
                        };

                        cursor.tps = new_tps.try_into().expect("does't fit in i64");
                    } else {
                        cursor.tps = 0;
                    }

                    cursor.head = new_head;
                    cursor.last_block_timestamp =
                        new_timestamp.try_into().expect("doesnt fit in i64");
                    cursor.last_pending_block_tx = None;
                    cursor.last_pending_block_contract_tx = None;

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
            QueryType::UpdateCursors(update_cursors) => {
                // Read all cursors from db
                let mut cursors: Vec<ContractCursor> =
                    sqlx::query_as("SELECT * FROM contracts").fetch_all(&mut **tx).await?;

                let new_head =
                    update_cursors.last_block_number.try_into().expect("doesn't fit in i64");
                let new_timestamp = update_cursors.pending_block_timestamp;

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
                            num_transactions
                        };

                        cursor.last_pending_block_contract_tx =
                            Some(felt_to_sql_string(&new_cursor.0));
                        cursor.tps = new_tps.try_into().expect("does't fit in i64");
                    } else {
                        cursor.tps = 0;
                    }
                    cursor.last_block_timestamp = update_cursors
                        .pending_block_timestamp
                        .try_into()
                        .expect("doesn't fit in i64");
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
            QueryType::SetEntity(entity) => {
                let row = query.fetch_one(&mut **tx).await.with_context(|| {
                    format!(
                        "Failed to execute query: {:?}, args: {:?}",
                        query_message.statement, query_message.arguments
                    )
                })?;
                let mut entity_updated = EntityUpdated::from_row(&row)?;
                entity_updated.updated_model = Some(entity);
                entity_updated.deleted = false;

                let optimistic_entity = OptimisticEntity {
                    id: entity_updated.id.clone(),
                    keys: entity_updated.keys.clone(),
                    event_id: entity_updated.event_id.clone(),
                    executed_at: entity_updated.executed_at,
                    created_at: entity_updated.created_at,
                    updated_at: entity_updated.updated_at,
                    updated_model: entity_updated.updated_model.clone(),
                    deleted: entity_updated.deleted,
                };
                SimpleBroker::publish(optimistic_entity);

                let broker_message = BrokerMessage::EntityUpdated(entity_updated);
                self.publish_queue.push(broker_message);
            }
            QueryType::DeleteEntity(entity) => {
                let delete_model = query.execute(&mut **tx).await.with_context(|| {
                    format!(
                        "Failed to execute query: {:?}, args: {:?}",
                        query_message.statement, query_message.arguments
                    )
                })?;
                if delete_model.rows_affected() == 0 {
                    return Ok(());
                }

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

                let optimistic_entity = OptimisticEntity {
                    id: entity_updated.id.clone(),
                    keys: entity_updated.keys.clone(),
                    event_id: entity_updated.event_id.clone(),
                    executed_at: entity_updated.executed_at,
                    created_at: entity_updated.created_at,
                    updated_at: entity_updated.updated_at,
                    updated_model: entity_updated.updated_model.clone(),
                    deleted: entity_updated.deleted,
                };
                SimpleBroker::publish(optimistic_entity);
                let broker_message = BrokerMessage::EntityUpdated(entity_updated);
                self.publish_queue.push(broker_message);
            }
            QueryType::RegisterModel => {
                let row = query.fetch_one(&mut **tx).await.with_context(|| {
                    format!(
                        "Failed to execute query: {:?}, args: {:?}",
                        query_message.statement, query_message.arguments
                    )
                })?;
                let model_registered = ModelRegistered::from_row(&row)?;
                self.publish_queue.push(BrokerMessage::ModelRegistered(model_registered));
            }
            QueryType::EventMessage(entity) => {
                let row = query.fetch_one(&mut **tx).await.with_context(|| {
                    format!(
                        "Failed to execute query: {:?}, args: {:?}",
                        query_message.statement, query_message.arguments
                    )
                })?;
                let mut event_message = EventMessageUpdated::from_row(&row)?;
                event_message.updated_model = Some(entity);

                let optimistic_event_message = OptimisticEventMessage {
                    id: event_message.id.clone(),
                    keys: event_message.keys.clone(),
                    event_id: event_message.event_id.clone(),
                    executed_at: event_message.executed_at,
                    created_at: event_message.created_at,
                    updated_at: event_message.updated_at,
                    updated_model: event_message.updated_model.clone(),
                };
                SimpleBroker::publish(optimistic_event_message);

                let broker_message = BrokerMessage::EventMessageUpdated(event_message);
                self.publish_queue.push(broker_message);
            }
            QueryType::StoreEvent => {
                let row = query.fetch_one(&mut **tx).await.with_context(|| {
                    format!(
                        "Failed to execute query: {:?}, args: {:?}",
                        query_message.statement, query_message.arguments
                    )
                })?;
                let event = EventEmitted::from_row(&row)?;
                self.publish_queue.push(BrokerMessage::EventEmitted(event));
            }
            QueryType::ApplyBalanceDiff(apply_balance_diff) => {
                debug!(target: LOG_TARGET, "Applying balance diff.");
                let instant = Instant::now();
                self.apply_balance_diff(apply_balance_diff).await?;
                debug!(target: LOG_TARGET, duration = ?instant.elapsed(), "Applied balance diff.");
            }
            QueryType::RegisterErc721Token(register_erc721_token) => {
                let semaphore = self.semaphore.clone();
                let artifacts_path = self.artifacts_path.clone();
                let provider = self.provider.clone();
                let res = sqlx::query_as::<_, (String, String)>(
                    "SELECT name, symbol FROM tokens WHERE contract_address = ?",
                )
                .bind(felt_to_sql_string(&register_erc721_token.contract_address))
                .fetch_one(&mut **tx)
                .await;

                // If we find a token already registered for this contract_address we dont need to
                // refetch the data since its same for all ERC721 tokens
                let (name, symbol) = match res {
                    Ok((name, symbol)) => {
                        debug!(
                            contract_address = %felt_to_sql_string(&register_erc721_token.contract_address),
                            "Token already registered for contract_address, so reusing fetched data",
                        );
                        (name, symbol)
                    }
                    Err(_) => {
                        // Fetch token information from the chain
                        let name = provider
                            .call(
                                FunctionCall {
                                    contract_address: register_erc721_token.contract_address,
                                    entry_point_selector: get_selector_from_name("name").unwrap(),
                                    calldata: vec![],
                                },
                                BlockId::Tag(BlockTag::Pending),
                            )
                            .await?;

                        // len = 1 => return value felt (i.e. legacy erc721 token)
                        // len > 1 => return value ByteArray (i.e. new erc721 token)
                        let name = if name.len() == 1 {
                            parse_cairo_short_string(&name[0]).unwrap()
                        } else {
                            ByteArray::cairo_deserialize(&name, 0)
                                .expect("Return value not ByteArray")
                                .to_string()
                                .expect("Return value not String")
                        };

                        let symbol = provider
                            .call(
                                FunctionCall {
                                    contract_address: register_erc721_token.contract_address,
                                    entry_point_selector: get_selector_from_name("symbol").unwrap(),
                                    calldata: vec![],
                                },
                                BlockId::Tag(BlockTag::Pending),
                            )
                            .await?;
                        let symbol = if symbol.len() == 1 {
                            parse_cairo_short_string(&symbol[0]).unwrap()
                        } else {
                            ByteArray::cairo_deserialize(&symbol, 0)
                                .expect("Return value not ByteArray")
                                .to_string()
                                .expect("Return value not String")
                        };

                        (name, symbol)
                    }
                };

                self.ongoing_futures.spawn(async move {
                    let permit = semaphore.acquire().await.unwrap();

                    let result = Self::process_register_erc721_token_query(
                        register_erc721_token,
                        &artifacts_path,
                        provider,
                        name,
                        symbol,
                    )
                    .await;

                    drop(permit);
                    result
                });
            }
            QueryType::RegisterErc20Token(register_erc20_token) => {
                let query = sqlx::query(
                    "INSERT INTO tokens (id, contract_address, name, symbol, decimals) VALUES (?, \
                     ?, ?, ?, ?)",
                )
                .bind(&register_erc20_token.token_id)
                .bind(felt_to_sql_string(&register_erc20_token.contract_address))
                .bind(&register_erc20_token.name)
                .bind(&register_erc20_token.symbol)
                .bind(register_erc20_token.decimals);

                query.execute(&mut **tx).await.with_context(|| {
                    format!(
                        "Failed to execute RegisterErc20Token query: {:?}",
                        register_erc20_token
                    )
                })?;
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
            QueryType::Other => {
                query.execute(&mut **tx).await.with_context(|| {
                    format!(
                        "Failed to execute query: {:?}, args: {:?}",
                        query_message.statement, query_message.arguments
                    )
                })?;
            }
        }

        Ok(())
    }

    async fn execute(&mut self, new_transaction: bool) -> Result<()> {
        if new_transaction {
            let transaction = mem::replace(&mut self.transaction, self.pool.begin().await?);
            transaction.commit().await?;
        }

        for message in self.publish_queue.drain(..) {
            send_broker_message(message);
        }

        while let Some(result) = self.ongoing_futures.join_next().await {
            let result = result??;
            self.handle_erc721_token_metadata(result).await?;
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

    async fn apply_balance_diff(
        &mut self,
        apply_balance_diff: ApplyBalanceDiffQuery,
    ) -> Result<()> {
        let erc_cache = apply_balance_diff.erc_cache;
        for ((contract_type, id_str), balance) in erc_cache.iter() {
            let id = id_str.split(FELT_DELIMITER).collect::<Vec<&str>>();
            match contract_type {
                ContractType::WORLD => unreachable!(),
                ContractType::ERC721 => {
                    // account_address/contract_address:id => ERC721
                    assert!(id.len() == 2);
                    let account_address = id[0];
                    let token_id = id[1];
                    let mid = token_id.split(":").collect::<Vec<&str>>();
                    let contract_address = mid[0];

                    self.apply_balance_diff_helper(
                        id_str,
                        account_address,
                        contract_address,
                        token_id,
                        balance,
                    )
                    .await
                    .with_context(|| "Failed to apply balance diff in apply_cache_diff")?;
                }
                ContractType::ERC20 => {
                    // account_address/contract_address/ => ERC20
                    assert!(id.len() == 3);
                    let account_address = id[0];
                    let contract_address = id[1];
                    let token_id = id[1];

                    self.apply_balance_diff_helper(
                        id_str,
                        account_address,
                        contract_address,
                        token_id,
                        balance,
                    )
                    .await
                    .with_context(|| "Failed to apply balance diff in apply_cache_diff")?;
                }
            }
        }

        Ok(())
    }

    async fn apply_balance_diff_helper(
        &mut self,
        id: &str,
        account_address: &str,
        contract_address: &str,
        token_id: &str,
        balance_diff: &I256,
    ) -> Result<()> {
        let tx = &mut self.transaction;
        let balance: Option<(String,)> =
            sqlx::query_as("SELECT balance FROM balances WHERE id = ?")
                .bind(id)
                .fetch_optional(&mut **tx)
                .await?;

        let mut balance = if let Some(balance) = balance {
            sql_string_to_u256(&balance.0)
        } else {
            U256::from(0u8)
        };

        if balance_diff.is_negative {
            if balance < balance_diff.value {
                dbg!(&balance_diff, balance, id);
            }
            balance -= balance_diff.value;
        } else {
            balance += balance_diff.value;
        }

        // write the new balance to the database
        sqlx::query(
            "INSERT OR REPLACE INTO balances (id, contract_address, account_address, token_id, \
             balance) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(id)
        .bind(contract_address)
        .bind(account_address)
        .bind(token_id)
        .bind(u256_to_sql_string(&balance))
        .execute(&mut **tx)
        .await?;

        Ok(())
    }

    async fn process_register_erc721_token_query(
        register_erc721_token: RegisterErc721TokenQuery,
        _artifacts_path: &Utf8PathBuf,
        provider: Arc<P>,
        name: String,
        symbol: String,
    ) -> Result<RegisterErc721TokenMetadata> {
        let token_uri = if let Ok(token_uri) = provider
            .call(
                FunctionCall {
                    contract_address: register_erc721_token.contract_address,
                    entry_point_selector: get_selector_from_name("token_uri").unwrap(),
                    calldata: vec![
                        register_erc721_token.actual_token_id.low().into(),
                        register_erc721_token.actual_token_id.high().into(),
                    ],
                },
                BlockId::Tag(BlockTag::Pending),
            )
            .await
        {
            token_uri
        } else if let Ok(token_uri) = provider
            .call(
                FunctionCall {
                    contract_address: register_erc721_token.contract_address,
                    entry_point_selector: get_selector_from_name("tokenURI").unwrap(),
                    calldata: vec![
                        register_erc721_token.actual_token_id.low().into(),
                        register_erc721_token.actual_token_id.high().into(),
                    ],
                },
                BlockId::Tag(BlockTag::Pending),
            )
            .await
        {
            token_uri
        } else {
            return Err(anyhow::anyhow!("Failed to fetch token_uri"));
        };

        let token_uri = if let Ok(byte_array) = ByteArray::cairo_deserialize(&token_uri, 0) {
            byte_array.to_string().expect("Return value not String")
        } else if let Ok(felt_array) = Vec::<Felt>::cairo_deserialize(&token_uri, 0) {
            felt_array
                .iter()
                .map(parse_cairo_short_string)
                .collect::<Result<Vec<String>, _>>()
                .map(|strings| strings.join(""))
                .map_err(|_| anyhow::anyhow!("Failed parsing Array<Felt> to String"))?
        } else {
            return Err(anyhow::anyhow!("token_uri is neither ByteArray nor Array<Felt>"));
        };

        let metadata = Self::fetch_metadata(&token_uri).await?;
        let metadata = serde_json::to_string(&metadata).context("Failed to serialize metadata")?;
        Ok(RegisterErc721TokenMetadata { query: register_erc721_token, metadata, name, symbol })
    }

    // given a uri which can be either http/https url or data uri, fetch the metadata erc721
    // metadata json schema
    async fn fetch_metadata(token_uri: &str) -> Result<serde_json::Value> {
        // Parse the token_uri

        match token_uri {
            uri if uri.starts_with("http") || uri.starts_with("https") => {
                // Fetch metadata from HTTP/HTTPS URL
                debug!(token_uri = %token_uri, "Fetching metadata from http/https URL");
                let client = Client::new();
                let response = client
                    .get(token_uri)
                    .send()
                    .await
                    .context("Failed to fetch metadata from URL")?;

                let bytes = response.bytes().await.context("Failed to read response bytes")?;
                let json: serde_json::Value = serde_json::from_slice(&bytes)
                    .context(format!("Failed to parse metadata JSON from response: {:?}", bytes))?;

                Ok(json)
            }
            uri if uri.starts_with("ipfs") => {
                let cid = uri.strip_prefix("ipfs://").unwrap();
                debug!(cid = %cid, "Fetching metadata from IPFS");
                let response = fetch_content_from_ipfs(cid, MAX_RETRY)
                    .await
                    .context("Failed to fetch metadata from IPFS")?;

                let json: serde_json::Value =
                    serde_json::from_slice(&response).context(format!(
                        "Failed to parse metadata JSON from IPFS: {:?}, data: {:?}",
                        cid, &response
                    ))?;

                Ok(json)
            }
            uri if uri.starts_with("data") => {
                // Parse and decode data URI
                debug!("Parsing metadata from data URI");
                trace!(data_uri = %token_uri);
                let data_url = DataUrl::process(token_uri).context("Failed to parse data URI")?;

                // Ensure the MIME type is JSON
                if data_url.mime_type() != &Mime::from_str("application/json").unwrap() {
                    return Err(anyhow::anyhow!("Data URI is not of JSON type"));
                }

                let decoded = data_url.decode_to_vec().context("Failed to decode data URI")?;

                let json: serde_json::Value = serde_json::from_slice(&decoded.0)
                    .context(format!("Failed to parse metadata JSON from data URI: {:?}", &uri))?;

                Ok(json)
            }
            uri => Err(anyhow::anyhow!("Unsupported URI scheme found in token URI: {}", uri)),
        }
    }

    async fn handle_erc721_token_metadata(
        &mut self,
        result: RegisterErc721TokenMetadata,
    ) -> Result<()> {
        let query = sqlx::query(
            "INSERT INTO tokens (id, contract_address, name, symbol, decimals, metadata) VALUES \
             (?, ?, ?, ?, ?, ?)",
        )
        .bind(&result.query.token_id)
        .bind(felt_to_sql_string(&result.query.contract_address))
        .bind(&result.name)
        .bind(&result.symbol)
        .bind(0)
        .bind(&result.metadata);

        query
            .execute(&mut *self.transaction)
            .await
            .with_context(|| format!("Failed to execute721Token query: {:?}", result))?;

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
    }
}
