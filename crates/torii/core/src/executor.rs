use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::mpsc::{unbounded_channel, Receiver, Sender, UnboundedReceiver, UnboundedSender};
use anyhow::{Context, Result};
use dojo_types::schema::Ty;
use sqlx::{FromRow, Pool, Sqlite};
use starknet::core::types::Felt;

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
    Commit,
    Other,
}

pub struct Executor {
    pool: Pool<Sqlite>,
    rx: UnboundedReceiver<QueryMessage>,
}

pub struct QueryMessage {
    pub statement: String,
    pub arguments: Vec<Argument>,
    pub query_type: QueryType,
}

impl Executor {
    pub fn new(pool: Pool<Sqlite>) -> (Self, UnboundedSender<QueryMessage>) {
        let (tx, rx) = unbounded_channel();
        (Executor { pool, rx }, tx)
    }

    pub async fn run(&mut self) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        let mut publish_queue = Vec::new();

        while let Some(msg) = self.rx.recv().await {
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
                    let row = query.fetch_one(&mut *tx).await.with_context(|| {
                        format!("Failed to execute query: {:?}, args: {:?}", statement, arguments)
                    })?;
                    let mut entity_updated = EntityUpdated::from_row(&row)?;
                    entity_updated.updated_model = Some(entity);
                    entity_updated.deleted = false;
                    let broker_message = BrokerMessage::EntityUpdated(entity_updated);
                    publish_queue.push(broker_message);
                }
                QueryType::DeleteEntity(entity) => {
                    let delete_model = query.execute(&mut *tx).await.with_context(|| {
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
                    .fetch_one(&mut *tx)
                    .await?;
                    let mut entity_updated = EntityUpdated::from_row(&row)?;
                    entity_updated.updated_model = Some(entity.entity);

                    let count = sqlx::query_scalar::<_, i64>(
                        "SELECT count(*) FROM entity_model WHERE entity_id = ?",
                    )
                    .bind(entity_updated.id.clone())
                    .fetch_one(&mut *tx)
                    .await?;

                    // Delete entity if all of its models are deleted
                    if count == 0 {
                        sqlx::query("DELETE FROM entities WHERE id = ?")
                            .bind(entity_updated.id.clone())
                            .execute(&mut *tx)
                            .await?;
                        entity_updated.deleted = true;
                    }

                    let broker_message = BrokerMessage::EntityUpdated(entity_updated);
                    publish_queue.push(broker_message);
                }
                QueryType::RegisterModel => {
                    let row = query.fetch_one(&mut *tx).await.with_context(|| {
                        format!("Failed to execute query: {:?}, args: {:?}", statement, arguments)
                    })?;
                    let model_registered = ModelRegistered::from_row(&row)?;
                    let broker_message = BrokerMessage::ModelRegistered(model_registered);
                    publish_queue.push(broker_message);
                }
                QueryType::StoreEvent => {
                    let row = query.fetch_one(&mut *tx).await.with_context(|| {
                        format!("Failed to execute query: {:?}, args: {:?}", statement, arguments)
                    })?;
                    let event = EventEmitted::from_row(&row)?;
                    let broker_message = BrokerMessage::EventEmitted(event);
                    publish_queue.push(broker_message);
                }
                QueryType::Commit => {
                    break;
                }
                QueryType::Other => {
                    query.execute(&mut *tx).await.with_context(|| {
                        format!("Failed to execute query: {:?}, args: {:?}", statement, arguments)
                    })?;
                }
            }
        }

        tx.commit().await?;

        for message in publish_queue {
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
