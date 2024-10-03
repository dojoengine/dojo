use std::collections::HashMap;
use std::mem;

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
use crate::sql::utils::{sql_string_to_u256, u256_to_sql_string, I256};
use crate::sql::FELT_DELIMITER;
use crate::types::{
    ContractType, Entity as EntityUpdated, Event as EventEmitted,
    EventMessage as EventMessageUpdated, Model as ModelRegistered,
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
pub enum QueryType {
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
            QueryType::SetEntity(entity) => {
                let row = query.fetch_one(&mut **tx).await.with_context(|| {
                    format!("Failed to execute query: {:?}, args: {:?}", statement, arguments)
                })?;
                let mut entity_updated = EntityUpdated::from_row(&row)?;
                entity_updated.updated_model = Some(entity);
                entity_updated.deleted = false;
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
        BrokerMessage::ModelRegistered(model) => SimpleBroker::publish(model),
        BrokerMessage::EntityUpdated(entity) => SimpleBroker::publish(entity),
        BrokerMessage::EventMessageUpdated(event) => SimpleBroker::publish(event),
        BrokerMessage::EventEmitted(event) => SimpleBroker::publish(event),
    }
}
