use std::collections::VecDeque;

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
pub struct QueryQueue {
    pool: Pool<Sqlite>,
    pub queue: VecDeque<(String, Vec<Argument>, QueryType)>,
    // publishes that are related to queries in the queue, they should be sent
    // after the queries are executed
    pub publish_queue: VecDeque<BrokerMessage>,
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
    Other,
}

impl QueryQueue {
    pub fn new(pool: Pool<Sqlite>) -> Self {
        QueryQueue { pool, queue: VecDeque::new(), publish_queue: VecDeque::new() }
    }

    pub fn enqueue<S: Into<String>>(
        &mut self,
        statement: S,
        arguments: Vec<Argument>,
        query_type: QueryType,
    ) {
        self.queue.push_back((statement.into(), arguments, query_type));
    }

    pub fn push_front<S: Into<String>>(
        &mut self,
        statement: S,
        arguments: Vec<Argument>,
        query_type: QueryType,
    ) {
        self.queue.push_front((statement.into(), arguments, query_type));
    }

    pub fn push_publish(&mut self, value: BrokerMessage) {
        self.publish_queue.push_back(value);
    }

    pub async fn execute_all(&mut self) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        while let Some((statement, arguments, query_type)) = self.queue.pop_front() {
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
                    self.push_publish(broker_message);
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
                    self.push_publish(broker_message);
                }
                QueryType::Other => {
                    query.execute(&mut *tx).await.with_context(|| {
                        format!("Failed to execute query: {:?}, args: {:?}", statement, arguments)
                    })?;
                }
            }
        }

        tx.commit().await?;

        while let Some(message) = self.publish_queue.pop_front() {
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
