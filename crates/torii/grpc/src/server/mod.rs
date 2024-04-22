pub mod logger;
pub mod subscriptions;

#[cfg(test)]
mod tests;

use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::str;
use std::str::FromStr;
use std::sync::Arc;

use dojo_types::schema::Ty;
use futures::Stream;
use proto::world::{
    MetadataRequest, MetadataResponse, RetrieveEntitiesRequest, RetrieveEntitiesResponse,
    RetrieveEventsRequest, RetrieveEventsResponse, SubscribeModelsRequest, SubscribeModelsResponse,
};
use sqlx::sqlite::SqliteRow;
use sqlx::{Pool, Row, Sqlite};
use starknet::core::utils::{cairo_short_string_to_felt, get_selector_from_name};
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::JsonRpcClient;
use starknet_crypto::FieldElement;
use tokio::net::TcpListener;
use tokio::sync::mpsc::Receiver;
use tokio_stream::wrappers::{ReceiverStream, TcpListenerStream};
use tonic::transport::Server;
use tonic::{Request, Response, Status};
use torii_core::cache::ModelCache;
use torii_core::error::{Error, ParseError, QueryError};
use torii_core::model::{build_sql_query, map_row_to_ty};

use self::subscriptions::entity::EntityManager;
use self::subscriptions::event_message::EventMessageManager;
use self::subscriptions::model_diff::{ModelDiffRequest, StateDiffManager};
use crate::proto::types::clause::ClauseType;
use crate::proto::world::world_server::WorldServer;
use crate::proto::world::{SubscribeEntitiesRequest, SubscribeEntityResponse};
use crate::proto::{self};
use crate::types::ComparisonOperator;

#[derive(Clone)]
pub struct DojoWorld {
    pool: Pool<Sqlite>,
    world_address: FieldElement,
    model_cache: Arc<ModelCache>,
    entity_manager: Arc<EntityManager>,
    event_message_manager: Arc<EventMessageManager>,
    state_diff_manager: Arc<StateDiffManager>,
}

impl DojoWorld {
    pub fn new(
        pool: Pool<Sqlite>,
        block_rx: Receiver<u64>,
        world_address: FieldElement,
        provider: Arc<JsonRpcClient<HttpTransport>>,
    ) -> Self {
        let model_cache = Arc::new(ModelCache::new(pool.clone()));
        let entity_manager = Arc::new(EntityManager::default());
        let event_message_manager = Arc::new(EventMessageManager::default());
        let state_diff_manager = Arc::new(StateDiffManager::default());

        tokio::task::spawn(subscriptions::model_diff::Service::new_with_block_rcv(
            block_rx,
            world_address,
            provider,
            Arc::clone(&state_diff_manager),
        ));

        tokio::task::spawn(subscriptions::entity::Service::new(
            pool.clone(),
            Arc::clone(&entity_manager),
            Arc::clone(&model_cache),
        ));

        Self {
            pool,
            world_address,
            model_cache,
            entity_manager,
            event_message_manager,
            state_diff_manager,
        }
    }
}

impl DojoWorld {
    pub async fn metadata(&self) -> Result<proto::types::WorldMetadata, Error> {
        let (world_address, world_class_hash, executor_address, executor_class_hash): (
            String,
            String,
            String,
            String,
        ) = sqlx::query_as(&format!(
            "SELECT world_address, world_class_hash, executor_address, executor_class_hash FROM \
             worlds WHERE id = '{:#x}'",
            self.world_address
        ))
        .fetch_one(&self.pool)
        .await?;

        let models: Vec<(String, String, String, String, u32, u32, String)> = sqlx::query_as(
            "SELECT id, name, class_hash, contract_address, packed_size, unpacked_size, layout \
             FROM models",
        )
        .fetch_all(&self.pool)
        .await?;

        let mut models_metadata = Vec::with_capacity(models.len());
        for model in models {
            let schema = self.model_cache.schema(&model.0).await?;
            models_metadata.push(proto::types::ModelMetadata {
                name: model.1,
                class_hash: model.2,
                contract_address: model.3,
                packed_size: model.4,
                unpacked_size: model.5,
                layout: hex::decode(&model.6).unwrap(),
                schema: serde_json::to_vec(&schema).unwrap(),
            });
        }

        Ok(proto::types::WorldMetadata {
            world_address,
            world_class_hash,
            executor_address,
            executor_class_hash,
            models: models_metadata,
        })
    }

    async fn entities_all(
        &self,
        limit: u32,
        offset: u32,
    ) -> Result<(Vec<proto::types::Entity>, u32), Error> {
        self.query_by_hashed_keys("entities", "entity_model", None, limit, offset).await
    }

    async fn events_all(&self, limit: u32, offset: u32) -> Result<Vec<proto::types::Event>, Error> {
        let query = r#"
            SELECT keys, data, transaction_hash
            FROM events
            ORDER BY id DESC
            LIMIT ? OFFSET ?
         "#
        .to_string();

        let row_events: Vec<(String, String, String)> =
            sqlx::query_as(&query).bind(limit).bind(offset).fetch_all(&self.pool).await?;
        row_events.iter().map(map_row_to_event).collect()
    }

    pub(crate) async fn query_by_hashed_keys(
        &self,
        table: &str,
        model_relation_table: &str,
        hashed_keys: Option<proto::types::HashedKeysClause>,
        limit: u32,
        offset: u32,
    ) -> Result<(Vec<proto::types::Entity>, u32), Error> {
        // TODO: use prepared statement for where clause
        let filter_ids = match hashed_keys {
            Some(hashed_keys) => {
                let ids = hashed_keys
                    .hashed_keys
                    .iter()
                    .map(|id| {
                        Ok(FieldElement::from_byte_slice_be(id)
                            .map(|id| format!("{table}.id = '{id:#x}'"))
                            .map_err(ParseError::FromByteSliceError)?)
                    })
                    .collect::<Result<Vec<_>, Error>>()?;

                format!("WHERE {}", ids.join(" OR "))
            }
            None => String::new(),
        };

        // count query that matches filter_ids
        let count_query = format!(
            r#"
                    SELECT count(*)
                    FROM {table}
                    {filter_ids}
                "#
        );
        // total count of rows without limit and offset
        let total_count: u32 = sqlx::query_scalar(&count_query).fetch_one(&self.pool).await?;

        // query to filter with limit and offset
        let query = format!(
            r#"
            SELECT {table}.id, group_concat({model_relation_table}.model_id) as model_ids
            FROM {table}
            JOIN {model_relation_table} ON {table}.id = {model_relation_table}.entity_id
            {filter_ids}
            GROUP BY {table}.id
            ORDER BY {table}.event_id DESC
            LIMIT ? OFFSET ?
         "#
        );

        let db_entities: Vec<(String, String)> =
            sqlx::query_as(&query).bind(limit).bind(offset).fetch_all(&self.pool).await?;

        let mut entities = Vec::with_capacity(db_entities.len());
        for (entity_id, models_str) in db_entities {
            let model_ids: Vec<&str> = models_str.split(',').collect();
            let schemas = self.model_cache.schemas(model_ids).await?;

            let entity_query = format!("{} WHERE {table}.id = ?", build_sql_query(&schemas)?);
            let row = sqlx::query(&entity_query).bind(&entity_id).fetch_one(&self.pool).await?;

            let models = schemas
                .iter()
                .map(|s| {
                    let mut struct_ty = s.as_struct().expect("schema should be struct").to_owned();
                    map_row_to_ty(&s.name(), &mut struct_ty, &row)?;

                    Ok(struct_ty.try_into().unwrap())
                })
                .collect::<Result<Vec<_>, Error>>()?;

            let hashed_keys = FieldElement::from_str(&entity_id).map_err(ParseError::FromStr)?;
            entities.push(proto::types::Entity {
                hashed_keys: hashed_keys.to_bytes_be().to_vec(),
                models,
            })
        }

        Ok((entities, total_count))
    }

    pub(crate) async fn query_by_keys(
        &self,
        table: &str,
        model_relation_table: &str,
        keys_clause: proto::types::KeysClause,
        limit: u32,
        offset: u32,
    ) -> Result<(Vec<proto::types::Entity>, u32), Error> {
        let keys = keys_clause
            .keys
            .iter()
            .map(|bytes| {
                if bytes.is_empty() {
                    return Ok("%".to_string());
                }
                Ok(FieldElement::from_byte_slice_be(bytes)
                    .map(|felt| format!("{:#x}", felt))
                    .map_err(ParseError::FromByteSliceError)?)
            })
            .collect::<Result<Vec<_>, Error>>()?;
        let keys_pattern = keys.join("/") + "/%";

        let count_query = format!(
            r#"
            SELECT count(*)
            FROM {table}
            JOIN {model_relation_table} ON {table}.id = {model_relation_table}.entity_id
            WHERE {model_relation_table}.model_id = '{:#x}' and {table}.keys LIKE ?
        "#,
            get_selector_from_name(&keys_clause.model).map_err(ParseError::NonAsciiName)?
        );

        // total count of rows that matches keys_pattern without limit and offset
        let total_count =
            sqlx::query_scalar(&count_query).bind(&keys_pattern).fetch_one(&self.pool).await?;

        let models_query = format!(
            r#"
            SELECT group_concat({model_relation_table}.model_id) as model_ids
            FROM {table}
            JOIN {model_relation_table} ON {table}.id = {model_relation_table}.entity_id
            WHERE {table}.keys LIKE ?
            GROUP BY {table}.id
            HAVING INSTR(model_ids, '{:#x}') > 0
            LIMIT 1
        "#,
            get_selector_from_name(&keys_clause.model).map_err(ParseError::NonAsciiName)?
        );
        let (models_str,): (String,) =
            sqlx::query_as(&models_query).bind(&keys_pattern).fetch_one(&self.pool).await?;

        println!("models_str: {}", models_str);

        let model_ids = models_str.split(',').collect::<Vec<&str>>();
        let schemas = self.model_cache.schemas(model_ids).await?;

        println!("schemas: {:?}", schemas);

        // query to filter with limit and offset
        let entities_query = format!(
            "{} WHERE {table}.keys LIKE ? ORDER BY {table}.event_id DESC LIMIT ? OFFSET ?",
            build_sql_query(&schemas)?
        );
        let db_entities = sqlx::query(&entities_query)
            .bind(&keys_pattern)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?;

        Ok((
            db_entities
                .iter()
                .map(|row| Self::map_row_to_entity(row, &schemas))
                .collect::<Result<Vec<_>, Error>>()?,
            total_count,
        ))
    }

    pub(crate) async fn events_by_keys(
        &self,
        keys_clause: proto::types::EventKeysClause,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<proto::types::Event>, Error> {
        let keys = keys_clause
            .keys
            .iter()
            .map(|bytes| {
                if bytes.is_empty() {
                    return Ok("%".to_string());
                }
                Ok(str::from_utf8(bytes).unwrap().to_string())
            })
            .collect::<Result<Vec<_>, Error>>()?;
        let keys_pattern = keys.join("/") + "/%";

        let events_query = r#"
            SELECT keys, data, transaction_hash
            FROM events
            WHERE keys LIKE ?
            ORDER BY id DESC
            LIMIT ? OFFSET ?
        "#
        .to_string();

        let row_events: Vec<(String, String, String)> = sqlx::query_as(&events_query)
            .bind(&keys_pattern)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?;

        row_events.iter().map(map_row_to_event).collect()
    }

    pub(crate) async fn query_by_member(
        &self,
        table: &str,
        model_relation_table: &str,
        member_clause: proto::types::MemberClause,
        _limit: u32,
        _offset: u32,
    ) -> Result<(Vec<proto::types::Entity>, u32), Error> {
        let comparison_operator = ComparisonOperator::from_repr(member_clause.operator as usize)
            .expect("invalid comparison operator");

        let value_type = member_clause
            .value
            .ok_or(QueryError::MissingParam("value".into()))?
            .value_type
            .ok_or(QueryError::MissingParam("value_type".into()))?;

        let comparison_value = match value_type {
            proto::types::value::ValueType::StringValue(string) => string,
            proto::types::value::ValueType::IntValue(int) => int.to_string(),
            proto::types::value::ValueType::UintValue(uint) => uint.to_string(),
            proto::types::value::ValueType::BoolValue(bool) => {
                if bool {
                    "1".to_string()
                } else {
                    "0".to_string()
                }
            }
            _ => return Err(QueryError::UnsupportedQuery.into()),
        };

        let models_query = format!(
            r#"
            SELECT group_concat({model_relation_table}.model_id) as model_ids
            FROM {table}
            JOIN {model_relation_table} ON {table}.id = {model_relation_table}.entity_id
            GROUP BY {table}.id
            HAVING INSTR(model_ids, '{:#x}') > 0
            LIMIT 1
        "#,
            get_selector_from_name(&member_clause.model).map_err(ParseError::NonAsciiName)?
        );
        let (models_str,): (String,) = sqlx::query_as(&models_query).fetch_one(&self.pool).await?;

        let model_ids = models_str.split(',').collect::<Vec<&str>>();
        let schemas = self.model_cache.schemas(model_ids).await?;

        let table_name = member_clause.model;
        let column_name = format!("external_{}", member_clause.member);
        let member_query = format!(
            "{} WHERE {table_name}.{column_name} {comparison_operator} ?",
            build_sql_query(&schemas)?
        );

        let db_entities =
            sqlx::query(&member_query).bind(comparison_value).fetch_all(&self.pool).await?;
        let entities_collection = db_entities
            .iter()
            .map(|row| Self::map_row_to_entity(row, &schemas))
            .collect::<Result<Vec<_>, Error>>()?;
        // Since there is not limit and offset, total_count is same as number of entities
        let total_count = entities_collection.len() as u32;
        Ok((entities_collection, total_count))
    }

    async fn query_by_composite(
        &self,
        _table: &str,
        _model_relation_table: &str,
        _composite: proto::types::CompositeClause,
        _limit: u32,
        _offset: u32,
    ) -> Result<(Vec<proto::types::Entity>, u32), Error> {
        // TODO: Implement
        Err(QueryError::UnsupportedQuery.into())
    }

    pub async fn model_metadata(&self, model: &str) -> Result<proto::types::ModelMetadata, Error> {
        // selector
        let model =
            format!("{:#x}", get_selector_from_name(model).map_err(ParseError::NonAsciiName)?);

        let (name, class_hash, contract_address, packed_size, unpacked_size, layout): (
            String,
            String,
            String,
            u32,
            u32,
            String,
        ) = sqlx::query_as(
            "SELECT name, class_hash, contract_address, packed_size, unpacked_size, layout FROM \
             models WHERE id = ?",
        )
        .bind(&model)
        .fetch_one(&self.pool)
        .await?;

        let schema = self.model_cache.schema(&model).await?;
        let layout = hex::decode(&layout).unwrap();

        Ok(proto::types::ModelMetadata {
            name,
            layout,
            class_hash,
            contract_address,
            packed_size,
            unpacked_size,
            schema: serde_json::to_vec(&schema).unwrap(),
        })
    }

    async fn subscribe_models(
        &self,
        models_keys: Vec<proto::types::KeysClause>,
    ) -> Result<Receiver<Result<proto::world::SubscribeModelsResponse, tonic::Status>>, Error> {
        let mut subs = Vec::with_capacity(models_keys.len());
        for keys in models_keys {
            let model = cairo_short_string_to_felt(&keys.model)
                .map_err(ParseError::CairoShortStringToFelt)?;

            let proto::types::ModelMetadata { packed_size, .. } =
                self.model_metadata(&keys.model).await?;

            subs.push(ModelDiffRequest {
                keys,
                model: subscriptions::model_diff::ModelMetadata {
                    name: model,
                    packed_size: packed_size as usize,
                },
            });
        }

        self.state_diff_manager.add_subscriber(subs).await
    }

    async fn subscribe_entities(
        &self,
        hashed_keys: Vec<FieldElement>,
    ) -> Result<Receiver<Result<proto::world::SubscribeEntityResponse, tonic::Status>>, Error> {
        self.entity_manager.add_subscriber(hashed_keys).await
    }

    async fn retrieve_entities(
        &self,
        query: proto::types::Query,
    ) -> Result<proto::world::RetrieveEntitiesResponse, Error> {
        let (entities, total_count) = match query.clause {
            None => self.entities_all(query.limit, query.offset).await?,
            Some(clause) => {
                let clause_type =
                    clause.clause_type.ok_or(QueryError::MissingParam("clause_type".into()))?;

                match clause_type {
                    ClauseType::HashedKeys(hashed_keys) => {
                        if hashed_keys.hashed_keys.is_empty() {
                            return Err(QueryError::MissingParam("ids".into()).into());
                        }

                        self.query_by_hashed_keys(
                            "entities",
                            "entity_model",
                            Some(hashed_keys),
                            query.limit,
                            query.offset,
                        )
                        .await?
                    }
                    ClauseType::Keys(keys) => {
                        if keys.keys.is_empty() {
                            return Err(QueryError::MissingParam("keys".into()).into());
                        }

                        if keys.model.is_empty() {
                            return Err(QueryError::MissingParam("model".into()).into());
                        }

                        self.query_by_keys(
                            "entities",
                            "entity_model",
                            keys,
                            query.limit,
                            query.offset,
                        )
                        .await?
                    }
                    ClauseType::Member(member) => {
                        self.query_by_member(
                            "entities",
                            "entity_model",
                            member,
                            query.limit,
                            query.offset,
                        )
                        .await?
                    }
                    ClauseType::Composite(composite) => {
                        self.query_by_composite(
                            "entities",
                            "entity_model",
                            composite,
                            query.limit,
                            query.offset,
                        )
                        .await?
                    }
                }
            }
        };

        Ok(RetrieveEntitiesResponse { entities, total_count })
    }

    async fn subscribe_event_messages(
        &self,
        hashed_keys: Vec<FieldElement>,
    ) -> Result<Receiver<Result<proto::world::SubscribeEntityResponse, tonic::Status>>, Error> {
        self.event_message_manager.add_subscriber(hashed_keys).await
    }

    async fn retrieve_event_messages(
        &self,
        query: proto::types::Query,
    ) -> Result<proto::world::RetrieveEntitiesResponse, Error> {
        let (entities, total_count) = match query.clause {
            None => self.entities_all(query.limit, query.offset).await?,
            Some(clause) => {
                let clause_type =
                    clause.clause_type.ok_or(QueryError::MissingParam("clause_type".into()))?;

                match clause_type {
                    ClauseType::HashedKeys(hashed_keys) => {
                        if hashed_keys.hashed_keys.is_empty() {
                            return Err(QueryError::MissingParam("ids".into()).into());
                        }

                        self.query_by_hashed_keys(
                            "event_messages",
                            "event_model",
                            Some(hashed_keys),
                            query.limit,
                            query.offset,
                        )
                        .await?
                    }
                    ClauseType::Keys(keys) => {
                        if keys.keys.is_empty() {
                            return Err(QueryError::MissingParam("keys".into()).into());
                        }

                        if keys.model.is_empty() {
                            return Err(QueryError::MissingParam("model".into()).into());
                        }

                        self.query_by_keys(
                            "event_messages",
                            "event_model",
                            keys,
                            query.limit,
                            query.offset,
                        )
                        .await?
                    }
                    ClauseType::Member(member) => {
                        self.query_by_member(
                            "event_messages",
                            "event_model",
                            member,
                            query.limit,
                            query.offset,
                        )
                        .await?
                    }
                    ClauseType::Composite(composite) => {
                        self.query_by_composite(
                            "event_messages",
                            "event_model",
                            composite,
                            query.limit,
                            query.offset,
                        )
                        .await?
                    }
                }
            }
        };

        Ok(RetrieveEntitiesResponse { entities, total_count })
    }

    async fn retrieve_events(
        &self,
        query: proto::types::EventQuery,
    ) -> Result<proto::world::RetrieveEventsResponse, Error> {
        let events = match query.keys {
            None => self.events_all(query.limit, query.offset).await?,
            Some(keys) => self.events_by_keys(keys, query.limit, query.offset).await?,
        };
        Ok(RetrieveEventsResponse { events })
    }

    fn map_row_to_entity(row: &SqliteRow, schemas: &[Ty]) -> Result<proto::types::Entity, Error> {
        let hashed_keys =
            FieldElement::from_str(&row.get::<String, _>("id")).map_err(ParseError::FromStr)?;
        let models = schemas
            .iter()
            .map(|schema| {
                let mut struct_ty = schema.as_struct().expect("schema should be struct").to_owned();
                map_row_to_ty(&schema.name(), &mut struct_ty, row)?;

                Ok(struct_ty.try_into().unwrap())
            })
            .collect::<Result<Vec<_>, Error>>()?;

        Ok(proto::types::Entity { hashed_keys: hashed_keys.to_bytes_be().to_vec(), models })
    }
}

fn process_event_field(data: &str) -> Vec<Vec<u8>> {
    data.trim_end_matches('/').split('/').map(|s| s.to_owned().into_bytes()).collect()
}

fn map_row_to_event(row: &(String, String, String)) -> Result<proto::types::Event, Error> {
    let keys = process_event_field(&row.0);
    let data = process_event_field(&row.1);
    let transaction_hash = row.2.to_owned().into_bytes();

    Ok(proto::types::Event { keys, data, transaction_hash })
}

type ServiceResult<T> = Result<Response<T>, Status>;
type SubscribeModelsResponseStream =
    Pin<Box<dyn Stream<Item = Result<SubscribeModelsResponse, Status>> + Send>>;
type SubscribeEntitiesResponseStream =
    Pin<Box<dyn Stream<Item = Result<SubscribeEntityResponse, Status>> + Send>>;

#[tonic::async_trait]
impl proto::world::world_server::World for DojoWorld {
    type SubscribeModelsStream = SubscribeModelsResponseStream;
    type SubscribeEntitiesStream = SubscribeEntitiesResponseStream;
    type SubscribeEventMessagesStream = SubscribeEntitiesResponseStream;

    async fn world_metadata(
        &self,
        _request: Request<MetadataRequest>,
    ) -> Result<Response<MetadataResponse>, Status> {
        let metadata = self.metadata().await.map_err(|e| match e {
            Error::Sql(sqlx::Error::RowNotFound) => Status::not_found("World not found"),
            e => Status::internal(e.to_string()),
        })?;

        Ok(Response::new(MetadataResponse { metadata: Some(metadata) }))
    }

    async fn subscribe_models(
        &self,
        request: Request<SubscribeModelsRequest>,
    ) -> ServiceResult<Self::SubscribeModelsStream> {
        let SubscribeModelsRequest { models_keys } = request.into_inner();
        let rx = self
            .subscribe_models(models_keys)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(Box::pin(ReceiverStream::new(rx)) as Self::SubscribeModelsStream))
    }

    async fn subscribe_entities(
        &self,
        request: Request<SubscribeEntitiesRequest>,
    ) -> ServiceResult<Self::SubscribeEntitiesStream> {
        let SubscribeEntitiesRequest { hashed_keys } = request.into_inner();
        let hashed_keys = hashed_keys
            .iter()
            .map(|id| {
                FieldElement::from_byte_slice_be(id)
                    .map_err(|e| Status::invalid_argument(e.to_string()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let rx = self
            .subscribe_entities(hashed_keys)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(Box::pin(ReceiverStream::new(rx)) as Self::SubscribeEntitiesStream))
    }

    async fn retrieve_entities(
        &self,
        request: Request<RetrieveEntitiesRequest>,
    ) -> Result<Response<RetrieveEntitiesResponse>, Status> {
        let query = request
            .into_inner()
            .query
            .ok_or_else(|| Status::invalid_argument("Missing query argument"))?;

        let entities =
            self.retrieve_entities(query).await.map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(entities))
    }

    async fn subscribe_event_messages(
        &self,
        request: Request<SubscribeEntitiesRequest>,
    ) -> ServiceResult<Self::SubscribeEntitiesStream> {
        let SubscribeEntitiesRequest { hashed_keys } = request.into_inner();
        let hashed_keys = hashed_keys
            .iter()
            .map(|id| {
                FieldElement::from_byte_slice_be(id)
                    .map_err(|e| Status::invalid_argument(e.to_string()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let rx = self
            .subscribe_event_messages(hashed_keys)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(Box::pin(ReceiverStream::new(rx)) as Self::SubscribeEntitiesStream))
    }

    async fn retrieve_event_messages(
        &self,
        request: Request<RetrieveEntitiesRequest>,
    ) -> Result<Response<RetrieveEntitiesResponse>, Status> {
        let query = request
            .into_inner()
            .query
            .ok_or_else(|| Status::invalid_argument("Missing query argument"))?;

        let entities = self
            .retrieve_event_messages(query)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(entities))
    }

    async fn retrieve_events(
        &self,
        request: Request<RetrieveEventsRequest>,
    ) -> Result<Response<RetrieveEventsResponse>, Status> {
        let query = request
            .into_inner()
            .query
            .ok_or_else(|| Status::invalid_argument("Missing query argument"))?;

        let events =
            self.retrieve_events(query).await.map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(events))
    }
}

pub async fn new(
    mut shutdown_rx: tokio::sync::broadcast::Receiver<()>,
    pool: &Pool<Sqlite>,
    block_rx: Receiver<u64>,
    world_address: FieldElement,
    provider: Arc<JsonRpcClient<HttpTransport>>,
) -> Result<
    (SocketAddr, impl Future<Output = Result<(), tonic::transport::Error>> + 'static),
    std::io::Error,
> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;

    let reflection = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(proto::world::FILE_DESCRIPTOR_SET)
        .build()
        .unwrap();

    let world = DojoWorld::new(pool.clone(), block_rx, world_address, provider);
    let server = WorldServer::new(world);

    let server_future = Server::builder()
        // GrpcWeb is over http1 so we must enable it.
        .accept_http1(true)
        .add_service(reflection)
        .add_service(tonic_web::enable(server))
        .serve_with_incoming_shutdown(TcpListenerStream::new(listener), async move {
            shutdown_rx.recv().await.map_or((), |_| ())
        });

    Ok((addr, server_future))
}
