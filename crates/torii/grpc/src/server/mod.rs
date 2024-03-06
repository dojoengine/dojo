pub mod logger;
pub mod subscriptions;

use std::collections::HashSet;
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
    RetrieveEventsRequest, RetrieveEventsResponse, SubscribeDiffsRequest, SubscribeDiffsResponse,
};
use sqlx::sqlite::SqliteRow;
use sqlx::{Pool, Row, Sqlite};
use starknet::core::utils::cairo_short_string_to_felt;
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
use torii_core::model::{build_sql_query, keys_to_pattern, map_row_to_ty};

use self::subscriptions::entity::EntityManager;
use self::subscriptions::model_diff::{ModelDiffRequest, StateDiffManager};
use crate::proto::types::entity_clause::ClauseType;
use crate::proto::world::world_server::WorldServer;
use crate::proto::world::{SubscribeEntitiesRequest, SubscribeEntityResponse};
use crate::proto::{self};
use crate::types::{LogicalOperator, MemberClause, ValueType};

#[derive(Clone)]
pub struct DojoWorld {
    pool: Pool<Sqlite>,
    world_address: FieldElement,
    model_cache: Arc<ModelCache>,
    entity_manager: Arc<EntityManager>,
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

        Self { pool, world_address, model_cache, entity_manager, state_diff_manager }
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

        let models: Vec<(String, String, String, u32, u32, String)> = sqlx::query_as(
            "SELECT name, class_hash, contract_address, packed_size, unpacked_size, layout FROM \
             models",
        )
        .fetch_all(&self.pool)
        .await?;

        let mut models_metadata = Vec::with_capacity(models.len());
        for model in models {
            let schema = self.model_cache.schema(&model.0).await?;
            models_metadata.push(proto::types::ModelMetadata {
                name: model.0,
                class_hash: model.1,
                contract_address: model.2,
                packed_size: model.3,
                unpacked_size: model.4,
                layout: hex::decode(&model.5).unwrap(),
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

    pub async fn model_metadata(&self, model: &str) -> Result<proto::types::ModelMetadata, Error> {
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
        .bind(model)
        .fetch_one(&self.pool)
        .await?;

        let schema = self.model_cache.schema(model).await?;
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

    async fn subscribe_diffs(
        &self,
        models_keys: Vec<proto::types::ModelDiffKeys>,
    ) -> Result<Receiver<Result<proto::world::SubscribeDiffsResponse, tonic::Status>>, Error> {
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
        entity_query: proto::types::EntityQuery,
    ) -> Result<proto::world::RetrieveEntitiesResponse, Error> {
        let (entities, total_count) = match entity_query.clause {
            None => self.entities_query(None, entity_query.limit, entity_query.offset).await?,
            Some(clause) => {
                let clause_type =
                    clause.clause_type.ok_or(QueryError::MissingParam("clause_type".into()))?;

                match clause_type {
                    ClauseType::HashedKeys(hashed) => {
                        self.entities_by_hashed_keys(
                            hashed,
                            entity_query.limit,
                            entity_query.offset,
                        )
                        .await?
                    }
                    ClauseType::Keys(keys) => {
                        self.entities_by_keys(keys, entity_query.limit, entity_query.offset).await?
                    }
                    ClauseType::Models(models) => {
                        self.entities_by_models(models, entity_query.limit, entity_query.offset)
                            .await?
                    }
                }
            }
        };

        Ok(RetrieveEntitiesResponse { entities, total_count })
    }

    async fn entities_by_keys(
        &self,
        keys_clause: proto::types::KeysClause,
        limit: u32,
        offset: u32,
    ) -> Result<(Vec<proto::types::Entity>, u32), Error> {
        let where_clause =
            format!("WHERE keys LIKE '{}'", Self::key_clause_to_pattern(keys_clause)?);

        self.entities_query(Some(where_clause), limit, offset).await
    }

    async fn entities_by_hashed_keys(
        &self,
        hashed_clause: proto::types::HashedKeysClause,
        limit: u32,
        offset: u32,
    ) -> Result<(Vec<proto::types::Entity>, u32), Error> {
        let ids: Vec<String> = hashed_clause
            .hashed_keys
            .iter()
            .map(|id| {
                Ok(FieldElement::from_byte_slice_be(id)
                    .map(|id| format!("entities.id = '{id:#x}'"))
                    .map_err(ParseError::FromByteSliceError)?)
            })
            .collect::<Result<Vec<_>, Error>>()?;

        let where_clause = format!("WHERE {}", ids.join(" OR "));
        self.entities_query(Some(where_clause), limit, offset).await
    }

    async fn entities_query(
        &self,
        where_clause: Option<String>,
        limit: u32,
        offset: u32,
    ) -> Result<(Vec<proto::types::Entity>, u32), Error> {
        let query = format!(
            r#"
            WITH filtered_entities AS (
                SELECT id, event_id
                FROM entities
                {}
            )
            SELECT 
                (SELECT count(*) FROM filtered_entities) AS total_count,
                fe.id, 
                group_concat(em.model_id) as model_names
            FROM filtered_entities fe
            JOIN entity_model em ON fe.id = em.entity_id
            GROUP BY fe.id
            ORDER BY fe.event_id DESC
            LIMIT ? OFFSET ?
            "#,
            where_clause.unwrap_or_default()
        );

        let results = sqlx::query(&query).bind(limit).bind(offset).fetch_all(&self.pool).await?;

        let total_count = results.first().map(|row| row.get("total_count")).unwrap_or(0);
        let mut entities = Vec::with_capacity(results.len());
        for row in results {
            let models_str: String = row.get("model_names");
            let entity_id: String = row.get("id");
            let model_names: HashSet<&str> = models_str.split(',').collect();
            let schemas = self.model_cache.schemas(model_names).await?;

            let entity_query =
                format!("{} WHERE entities.id = ?", build_sql_query(&schemas, None)?);
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

    async fn entities_by_models(
        &self,
        models_clause: proto::types::ModelsClause,
        limit: u32,
        offset: u32,
    ) -> Result<(Vec<proto::types::Entity>, u32), Error> {
        let model_names = models_clause
            .members
            .iter()
            .map(|clause| clause.model.as_str())
            .collect::<HashSet<&str>>();

        let model_filters = models_clause
            .members
            .iter()
            .map(|clause| {
                let member_clause = MemberClause::try_from(clause.clone())
                    .map_err(|_| QueryError::MissingParam("clause params".into()))?;

                let value = match member_clause.value.value_type {
                    ValueType::String(s) => format!("'{}'", s),
                    ValueType::Int(i) => i.to_string(),
                    ValueType::UInt(u) => u.to_string(),
                    ValueType::Bool(b) => b.to_string(),
                    ValueType::Bytes(bytes) => FieldElement::from_byte_slice_be(&bytes)
                        .map(|fe| format!("'0x{:064x}'", fe))
                        .map_err(ParseError::FromByteSliceError)?,
                };
                let table_name = member_clause.model;
                let column_name = format!("external_{}", member_clause.member);
                let op = member_clause.operator;

                Ok(format!("{table_name}.{column_name} {op} {value}"))
            })
            .collect::<Result<Vec<_>, Error>>()?;

        let schemas = self.model_cache.schemas(model_names).await?;
        let base_query = build_sql_query(&schemas, None)?;
        let base_query_with_total_count = build_sql_query(
            &schemas,
            Some("(SELECT count(*) FROM filtered_entities) AS total_count"),
        )?;
        let operator = LogicalOperator::from_repr(models_clause.operator as usize)
            .unwrap_or(LogicalOperator::And);
        let where_clause = format!("WHERE {}", model_filters.join(&format!(" {} ", operator)));
        let query = format!(
            r#"
            WITH filtered_entities AS (
                {base_query} {where_clause} 
            )
            {base_query_with_total_count}
            ORDER BY entities.event_id DESC 
            LIMIT ? OFFSET ?
            "#
        );

        let db_entities =
            sqlx::query(&query).bind(limit).bind(offset).fetch_all(&self.pool).await?;
        let total_count = db_entities.first().map(|row| row.get("total_count")).unwrap_or(0);
        let entities_collection = db_entities
            .iter()
            .map(|row| Self::map_row_to_entity(row, &schemas))
            .collect::<Result<Vec<_>, Error>>()?;

        Ok((entities_collection, total_count))
    }

    async fn retrieve_events(
        &self,
        event_query: proto::types::EventQuery,
    ) -> Result<proto::world::RetrieveEventsResponse, Error> {
        let where_clause = match event_query.keys {
            None => String::new(),
            Some(keys) => {
                format!("WHERE keys LIKE '{}'", Self::key_clause_to_pattern(keys)?)
            }
        };

        let query = format!(
            r#"
            SELECT keys, data, transaction_hash
            FROM events
            {}
            ORDER BY id DESC
            LIMIT ? OFFSET ?
            "#,
            where_clause
        );

        let row_events = sqlx::query(&query)
            .bind(event_query.limit)
            .bind(event_query.offset)
            .fetch_all(&self.pool)
            .await?;

        let events =
            row_events.iter().map(Self::map_row_to_event).collect::<Result<Vec<_>, _>>()?;

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

    fn map_row_to_event(row: &SqliteRow) -> Result<proto::types::Event, Error> {
        let keys = Self::process_event_field(&row.get::<String, _>("keys"))?;
        let data = Self::process_event_field(&row.get::<String, _>("data"))?;
        let transaction_hash = FieldElement::from_str(&row.get::<String, _>("transaction_hash"))
            .map_err(ParseError::FromStr)?
            .to_bytes_be()
            .to_vec();

        Ok(proto::types::Event { keys, data, transaction_hash })
    }

    fn process_event_field(data: &str) -> Result<Vec<Vec<u8>>, ParseError> {
        data.trim_end_matches('/')
            .split('/')
            .map(|s| {
                FieldElement::from_str(s)
                    .map_err(ParseError::FromStr)
                    .map(|fe| fe.to_bytes_be().to_vec())
            })
            .collect()
    }

    fn key_clause_to_pattern(keys_clause: proto::types::KeysClause) -> Result<String, Error> {
        let keys = keys_clause
            .keys
            .iter()
            .map(|bytes| {
                if bytes.is_empty() {
                    return Ok("*".to_string());
                }
                Ok(FieldElement::from_byte_slice_be(bytes)
                    .map(|felt| format!("{:#x}", felt))
                    .map_err(ParseError::FromByteSliceError)?)
            })
            .collect::<Result<Vec<_>, Error>>()?;

        Ok(keys_to_pattern(&keys, false))
    }
}

type ServiceResult<T> = Result<Response<T>, Status>;
type SubscribeDiffsResponseStream =
    Pin<Box<dyn Stream<Item = Result<SubscribeDiffsResponse, Status>> + Send>>;
type SubscribeEntitiesResponseStream =
    Pin<Box<dyn Stream<Item = Result<SubscribeEntityResponse, Status>> + Send>>;

#[tonic::async_trait]
impl proto::world::world_server::World for DojoWorld {
    type SubscribeDiffsStream = SubscribeDiffsResponseStream;
    type SubscribeEntitiesStream = SubscribeEntitiesResponseStream;

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

    async fn subscribe_diffs(
        &self,
        request: Request<SubscribeDiffsRequest>,
    ) -> ServiceResult<Self::SubscribeDiffsStream> {
        let SubscribeDiffsRequest { models_keys } = request.into_inner();
        let rx =
            self.subscribe_diffs(models_keys).await.map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(Box::pin(ReceiverStream::new(rx)) as Self::SubscribeDiffsStream))
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
