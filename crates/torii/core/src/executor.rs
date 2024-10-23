use std::collections::HashMap;
use std::mem;
use std::str::FromStr;

use anyhow::{Context, Result};
use dojo_types::schema::{Struct, Ty};
use sqlx::query::Query;
use sqlx::sqlite::SqliteArguments;
use sqlx::{FromRow, Pool, Sqlite, Transaction};
use starknet::core::types::{Felt, U256};
use tokio::sync::broadcast::{Receiver, Sender};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::sync::oneshot;
use tokio::time::Instant;
use tracing::{debug, error};

use crate::simple_broker::SimpleBroker;
use crate::sql::utils::{felt_to_sql_string, sql_string_to_u256, u256_to_sql_string, I256};
use crate::sql::FELT_DELIMITER;
use crate::types::{
    ContractCursor, ContractType, Entity as EntityUpdated, Event as EventEmitted,
    EventMessage as EventMessageUpdated, Model as ModelRegistered, OptimisticEntity,
    OptimisticEventMessage,
};

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
pub enum QueryType {
    SetHead(SetHeadQuery),
    ResetCursors(ResetCursorsQuery),
    UpdateCursors(UpdateCursorsQuery),
    SetEntity(Ty),
    DeleteEntity(DeleteEntityQuery),
    EventMessage(Ty),
    ApplyBalanceDiff(ApplyBalanceDiffQuery),
    RegisterModel,
    StoreEvent,
    Execute,
    Other,
}

#[derive(Debug)]
pub struct Executor<'c> {
    // Queries should use `transaction` instead of `pool`
    // This `pool` is only used to create a new `transaction`
    pool: Pool<Sqlite>,
    transaction: Transaction<'c, Sqlite>,
    publish_queue: Vec<BrokerMessage>,
    rx: UnboundedReceiver<QueryMessage>,
    shutdown_rx: Receiver<()>,
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
}

impl<'c> Executor<'c> {
    pub async fn new(
        pool: Pool<Sqlite>,
        shutdown_tx: Sender<()>,
    ) -> Result<(Self, UnboundedSender<QueryMessage>)> {
        let (tx, rx) = unbounded_channel();
        let transaction = pool.begin().await?;
        let publish_queue = Vec::new();
        let shutdown_rx = shutdown_tx.subscribe();

        Ok((Executor { pool, transaction, publish_queue, rx, shutdown_rx }, tx))
    }

    pub async fn run(&mut self) -> Result<()> {
        loop {
            tokio::select! {
                _ = self.shutdown_rx.recv() => {
                    debug!(target: LOG_TARGET, "Shutting down executor");
                    break Ok(());
                }
                Some(msg) = self.rx.recv() => {
                    let QueryMessage { statement, arguments, query_type, tx } = msg;
                    let mut query = sqlx::query(&statement);

                    for arg in &arguments {
                        query = match arg {
                            Argument::Null => query.bind(None::<String>),
                            Argument::Int(integer) => query.bind(integer),
                            Argument::Bool(bool) => query.bind(bool),
                            Argument::String(string) => query.bind(string),
                            Argument::FieldElement(felt) => query.bind(format!("{:#x}", felt)),
                        }
                    }

                    match self.handle_query_type(query, query_type.clone(), &statement, &arguments, tx).await {
                        Ok(()) => {},
                        Err(e) => {
                            error!(target: LOG_TARGET, r#type = ?query_type, error = %e, "Failed to execute query.");
                        }
                    }
                }
            }
        }
    }

    async fn handle_query_type<'a>(
        &mut self,
        query: Query<'a, Sqlite, SqliteArguments<'a>>,
        query_type: QueryType,
        statement: &str,
        arguments: &[Argument],
        sender: Option<oneshot::Sender<Result<()>>>,
    ) -> Result<()> {
        let tx = &mut self.transaction;

        match query_type {
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
                    format!("Failed to execute query: {:?}, args: {:?}", statement, arguments)
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
                    format!("Failed to execute query: {:?}, args: {:?}", statement, arguments)
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
                    format!("Failed to execute query: {:?}, args: {:?}", statement, arguments)
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
                    format!("Failed to execute query: {:?}, args: {:?}", statement, arguments)
                })?;
                let model_registered = ModelRegistered::from_row(&row)?;
                self.publish_queue.push(BrokerMessage::ModelRegistered(model_registered));
            }
            QueryType::EventMessage(entity) => {
                let row = query.fetch_one(&mut **tx).await.with_context(|| {
                    format!("Failed to execute query: {:?}, args: {:?}", statement, arguments)
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
                    format!("Failed to execute query: {:?}, args: {:?}", statement, arguments)
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
            QueryType::Execute => {
                debug!(target: LOG_TARGET, "Executing query.");
                let instant = Instant::now();
                let res = self.execute().await;
                debug!(target: LOG_TARGET, duration = ?instant.elapsed(), "Executed query.");

                if let Some(sender) = sender {
                    sender
                        .send(res)
                        .map_err(|_| anyhow::anyhow!("Failed to send execute result"))?;
                } else {
                    res?;
                }
            }
            QueryType::Other => {
                query.execute(&mut **tx).await.with_context(|| {
                    format!("Failed to execute query: {:?}, args: {:?}", statement, arguments)
                })?;
            }
        }

        Ok(())
    }

    async fn execute(&mut self) -> Result<()> {
        let transaction = mem::replace(&mut self.transaction, self.pool.begin().await?);
        transaction.commit().await?;

        for message in self.publish_queue.drain(..) {
            send_broker_message(message);
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
