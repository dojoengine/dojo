use std::collections::VecDeque;
use std::mem;

use anyhow::{Context, Result};
use dojo_types::schema::Ty;
use sqlx::{FromRow, Pool, Sqlite, Transaction};
use starknet::core::types::Felt;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

use crate::simple_broker::SimpleBroker;
use crate::types::{
    Entity as EntityUpdated, Event as EventEmitted, EventMessage as EventMessageUpdated,
    Model as ModelRegistered,
};

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
    pub entity: Ty,
}

#[derive(Debug, Clone)]
pub enum QueryType {
    SetEntity(Ty),
    DeleteEntity(DeleteEntityQuery),
    RegisterModel,
    StoreEvent,
    Execute,
    Other,
}

pub struct Executor<'c> {
    pool: Pool<Sqlite>,
    transaction: Transaction<'c, Sqlite>,
    publish_queue: Vec<BrokerMessage>,
    rx: UnboundedReceiver<QueryMessage>,
}

pub struct QueryMessage {
    pub statement: String,
    pub arguments: Vec<Argument>,
    pub query_type: QueryType,
}

impl<'c> Executor<'c> {
    pub async fn new(pool: Pool<Sqlite>) -> Result<(Self, UnboundedSender<QueryMessage>)> {
        let (tx, rx) = unbounded_channel();
        let transaction = pool.begin().await?;
        let publish_queue = Vec::new();

        Ok((Executor { pool, transaction, publish_queue, rx }, tx))
    }

    pub async fn run(&mut self) -> Result<()> {
        while let Some(msg) = self.rx.recv().await {
            let tx = &mut self.transaction;
            let QueryMessage { statement, arguments, query_type } = msg;
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
                        continue;
                    }

                    let row = sqlx::query(
                        "UPDATE entities SET updated_at=CURRENT_TIMESTAMP, executed_at=?, \
                         event_id=? WHERE id = ? RETURNING *",
                    )
                    .bind(entity.block_timestamp)
                    .bind(entity.event_id)
                    .bind(entity.entity_id)
                    .fetch_one(&mut **tx)
                    .await?;
                    let mut entity_updated = EntityUpdated::from_row(&row)?;
                    entity_updated.updated_model = Some(entity.entity);

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
                    let broker_message = BrokerMessage::ModelRegistered(model_registered);
                    self.publish_queue.push(broker_message);
                }
                QueryType::StoreEvent => {
                    let row = query.fetch_one(&mut **tx).await.with_context(|| {
                        format!("Failed to execute query: {:?}, args: {:?}", statement, arguments)
                    })?;
                    let event = EventEmitted::from_row(&row)?;
                    let broker_message = BrokerMessage::EventEmitted(event);
                    self.publish_queue.push(broker_message);
                }
                QueryType::Execute => {
                    self.execute().await?;
                }
                QueryType::Other => {
                    query.execute(&mut **tx).await.with_context(|| {
                        format!("Failed to execute query: {:?}, args: {:?}", statement, arguments)
                    })?;
                }
            }
        }

        Ok(())
    }

    pub async fn execute(&mut self) -> Result<()> {
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
        BrokerMessage::ModelRegistered(model) => SimpleBroker::publish(model),
        BrokerMessage::EntityUpdated(entity) => SimpleBroker::publish(entity),
        BrokerMessage::EventMessageUpdated(event) => SimpleBroker::publish(event),
        BrokerMessage::EventEmitted(event) => SimpleBroker::publish(event),
    }
}
