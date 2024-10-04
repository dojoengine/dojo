use std::mem;

use anyhow::{Context, Result};
use dojo_types::schema::{Struct, Ty};
use sqlx::query::Query;
use sqlx::sqlite::SqliteArguments;
use sqlx::{FromRow, Pool, Sqlite, Transaction};
use starknet::core::types::Felt;
use tokio::sync::broadcast::{Receiver, Sender};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::sync::oneshot;
use tokio::time::Instant;
use tracing::{debug, error};

use crate::simple_broker::SimpleBroker;
use crate::types::{
    Contract as ContractUpdated, Entity as EntityUpdated, Event as EventEmitted,
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
    SetHead(ContractUpdated),
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
pub struct SetHeadQuery {
    pub head: u64,
    pub last_block_timestamp: u64,
    pub txns_count: u64,
    pub contract_address: Felt,
}

#[derive(Debug, Clone)]
pub enum QueryType {
    SetHead(SetHeadQuery),
    SetEntity(Ty),
    DeleteEntity(DeleteEntityQuery),
    EventMessage(Ty),
    RegisterModel,
    StoreEvent,
    Execute,
    Other,
}

#[derive(Debug)]
pub struct Executor<'c> {
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

                sqlx::query("UPDATE contracts SET tps = ? WHERE id = ?")
                    .bind(tps as i64)
                    .bind(format!("{:#x}", set_head.contract_address))
                    .execute(&mut **tx)
                    .await?;

                self.publish_queue.push(BrokerMessage::SetHead(ContractUpdated {
                    head: set_head.head,
                    tps,
                    last_block_timestamp: set_head.last_block_timestamp,
                    contract_address: set_head.contract_address,
                }));
            }
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
