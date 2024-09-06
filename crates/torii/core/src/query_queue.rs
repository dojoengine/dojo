use std::collections::VecDeque;

use anyhow::{Context, Result};
use dojo_types::schema::Ty;
use sqlx::{Executor, Pool, Sqlite};
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
    pub queue: VecDeque<(String, Vec<Argument>)>,
    // publishes that are related to queries in the queue, they should be sent
    // after the queries are executed
    pub publish_queue: VecDeque<BrokerMessage>,
    pub publish_queries: VecDeque<(String, Vec<Argument>, QueryType)>,
}

#[derive(Debug, Clone)]
pub enum QueryType {
    SetEntity(Ty),
}

impl QueryQueue {
    pub fn new(pool: Pool<Sqlite>) -> Self {
        QueryQueue {
            pool,
            queue: VecDeque::new(),
            publish_queue: VecDeque::new(),
            publish_queries: VecDeque::new(),
        }
    }

    pub fn enqueue<S: Into<String>>(&mut self, statement: S, arguments: Vec<Argument>) {
        self.queue.push_back((statement.into(), arguments));
    }

    pub fn push_front<S: Into<String>>(&mut self, statement: S, arguments: Vec<Argument>) {
        self.queue.push_front((statement.into(), arguments));
    }

    pub fn push_publish(&mut self, value: BrokerMessage) {
        self.publish_queue.push_back(value);
    }

    pub fn push_publish_query(
        &mut self,
        statement: String,
        arguments: Vec<Argument>,
        query_type: QueryType,
    ) {
        self.publish_queries.push_back((statement, arguments, query_type));
    }

    pub async fn execute_all(&mut self) -> Result<u64> {
        let mut total_affected = 0_u64;
        let mut tx = self.pool.begin().await?;

        while let Some((statement, arguments)) = self.queue.pop_front() {
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

            total_affected += tx
                .execute(query)
                .await
                .with_context(|| format!("Failed to execute query: {}", statement))?
                .rows_affected();
        }

        tx.commit().await?;

        while let Some(message) = self.publish_queue.pop_front() {
            send_broker_message(message);
        }

        while let Some((statement, arguments, query_type)) = self.publish_queries.pop_front() {
            let mut query = sqlx::query_as(&statement);
            for arg in &arguments {
                query = match arg {
                    Argument::Null => query.bind(None::<String>),
                    Argument::Int(integer) => query.bind(integer),
                    Argument::Bool(bool) => query.bind(bool),
                    Argument::String(string) => query.bind(string),
                    Argument::FieldElement(felt) => query.bind(format!("{:#x}", felt)),
                }
            }

            let broker_message = match query_type {
                QueryType::SetEntity(entity) => {
                    let mut result: EntityUpdated = query
                        .fetch_one(&self.pool)
                        .await
                        .with_context(|| format!("Failed to fetch entity: {}", statement))?;
                    result.updated_model = Some(entity);
                    result.deleted = false;
                    BrokerMessage::EntityUpdated(result)
                }
            };
            send_broker_message(broker_message);
        }

        Ok(total_affected)
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
