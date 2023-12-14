pub mod logger;
pub mod subscriptions;

use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::str::FromStr;
use std::sync::Arc;

use dojo_types::schema::Ty;
use futures::Stream;
use proto::world::{
    MetadataRequest, MetadataResponse, RetrieveEntitiesRequest, RetrieveEntitiesResponse,
    SubscribeModelsRequest, SubscribeModelsResponse,
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
use torii_core::model::{build_sql_query, map_row_to_ty};

use self::subscriptions::entity::EntityManager;
use self::subscriptions::model_diff::{ModelDiffRequest, StateDiffManager};
use crate::proto::types::clause::ClauseType;
use crate::proto::world::world_server::WorldServer;
use crate::proto::world::{SubscribeEntitiesRequest, SubscribeEntityResponse};
use crate::proto::{self};

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

        let models: Vec<(String, String, u32, u32, String)> = sqlx::query_as(
            "SELECT name, class_hash, packed_size, unpacked_size, layout FROM models",
        )
        .fetch_all(&self.pool)
        .await?;

        let mut models_metadata = Vec::with_capacity(models.len());
        for model in models {
            let schema = self.model_cache.schema(&model.0).await?;
            models_metadata.push(proto::types::ModelMetadata {
                name: model.0,
                class_hash: model.1,
                packed_size: model.2,
                unpacked_size: model.3,
                layout: hex::decode(&model.4).unwrap(),
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
    ) -> Result<Vec<proto::types::Entity>, Error> {
        self.entities_by_ids(None, limit, offset).await
    }

    async fn entities_by_ids(
        &self,
        ids_clause: Option<proto::types::IdsClause>,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<proto::types::Entity>, Error> {
        // TODO: use prepared statement for where clause
        let filter_ids = match ids_clause {
            Some(ids_clause) => {
                let ids = ids_clause
                    .ids
                    .iter()
                    .map(|id| {
                        Ok(FieldElement::from_byte_slice_be(id)
                            .map(|id| format!("entities.id = '{id:#x}'"))
                            .map_err(ParseError::FromByteSliceError)?)
                    })
                    .collect::<Result<Vec<_>, Error>>()?;

                format!("WHERE {}", ids.join(" OR "))
            }
            None => String::new(),
        };

        let query = format!(
            r#"
            SELECT entities.id, group_concat(entity_model.model_id) as model_names
            FROM entities
            JOIN entity_model ON entities.id = entity_model.entity_id
            {filter_ids}
            GROUP BY entities.id
            ORDER BY entities.event_id DESC
            LIMIT ? OFFSET ?
         "#
        );

        let db_entities: Vec<(String, String)> =
            sqlx::query_as(&query).bind(limit).bind(offset).fetch_all(&self.pool).await?;

        let mut entities = Vec::with_capacity(db_entities.len());
        for (entity_id, models_str) in db_entities {
            let model_names: Vec<&str> = models_str.split(',').collect();
            let schemas = self.model_cache.schemas(model_names).await?;

            let entity_query = format!("{} WHERE entities.id = ?", build_sql_query(&schemas)?);
            let row = sqlx::query(&entity_query).bind(&entity_id).fetch_one(&self.pool).await?;

            let models = schemas
                .iter()
                .map(|s| {
                    let mut struct_ty = s.as_struct().expect("schema should be struct").to_owned();
                    map_row_to_ty(&s.name(), &mut struct_ty, &row)?;

                    Ok(struct_ty.try_into().unwrap())
                })
                .collect::<Result<Vec<_>, Error>>()?;

            let id = FieldElement::from_str(&entity_id).map_err(ParseError::FromStr)?;
            entities.push(proto::types::Entity { id: id.to_bytes_be().to_vec(), models })
        }

        Ok(entities)
    }

    async fn entities_by_keys(
        &self,
        keys_clause: proto::types::KeysClause,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<proto::types::Entity>, Error> {
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

        let models_query = format!(
            r#"
            SELECT group_concat(entity_model.model_id) as model_names
            FROM entities
            JOIN entity_model ON entities.id = entity_model.entity_id
            WHERE entities.keys LIKE ?
            GROUP BY entities.id
            HAVING model_names REGEXP '(^|,){}(,|$)'
            LIMIT 1
        "#,
            keys_clause.model
        );
        let (models_str,): (String,) =
            sqlx::query_as(&models_query).bind(&keys_pattern).fetch_one(&self.pool).await?;

        let model_names = models_str.split(',').collect::<Vec<&str>>();
        let schemas = self.model_cache.schemas(model_names).await?;

        let entities_query = format!(
            "{} WHERE entities.keys LIKE ? ORDER BY entities.event_id DESC LIMIT ? OFFSET ?",
            build_sql_query(&schemas)?
        );
        let db_entities = sqlx::query(&entities_query)
            .bind(&keys_pattern)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?;

        db_entities.iter().map(|row| Self::map_row_to_entity(row, &schemas)).collect()
    }

    async fn entities_by_attribute(
        &self,
        _attribute: proto::types::MemberClause,
        _limit: u32,
        _offset: u32,
    ) -> Result<Vec<proto::types::Entity>, Error> {
        // TODO: Implement
        Err(QueryError::UnsupportedQuery.into())
    }

    async fn entities_by_composite(
        &self,
        _composite: proto::types::CompositeClause,
        _limit: u32,
        _offset: u32,
    ) -> Result<Vec<proto::types::Entity>, Error> {
        // TODO: Implement
        Err(QueryError::UnsupportedQuery.into())
    }

    pub async fn model_metadata(&self, model: &str) -> Result<proto::types::ModelMetadata, Error> {
        let (name, class_hash, packed_size, unpacked_size, layout): (
            String,
            String,
            u32,
            u32,
            String,
        ) = sqlx::query_as(
            "SELECT name, class_hash, packed_size, unpacked_size, layout FROM models WHERE id = ?",
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
        ids: Vec<FieldElement>,
    ) -> Result<Receiver<Result<proto::world::SubscribeEntityResponse, tonic::Status>>, Error> {
        self.entity_manager.add_subscriber(ids).await
    }

    async fn retrieve_entities(
        &self,
        query: proto::types::Query,
    ) -> Result<proto::world::RetrieveEntitiesResponse, Error> {
        let entities = match query.clause {
            None => self.entities_all(query.limit, query.offset).await?,
            Some(clause) => {
                let clause_type =
                    clause.clause_type.ok_or(QueryError::MissingParam("clause_type".into()))?;

                match clause_type {
                    ClauseType::Ids(ids) => {
                        if ids.ids.is_empty() {
                            return Err(QueryError::MissingParam("ids".into()).into());
                        }

                        self.entities_by_ids(Some(ids), query.limit, query.offset).await?
                    }
                    ClauseType::Keys(keys) => {
                        if keys.keys.is_empty() {
                            return Err(QueryError::MissingParam("keys".into()).into());
                        }

                        if keys.model.is_empty() {
                            return Err(QueryError::MissingParam("model".into()).into());
                        }

                        self.entities_by_keys(keys, query.limit, query.offset).await?
                    }
                    ClauseType::Member(attribute) => {
                        self.entities_by_attribute(attribute, query.limit, query.offset).await?
                    }
                    ClauseType::Composite(composite) => {
                        self.entities_by_composite(composite, query.limit, query.offset).await?
                    }
                }
            }
        };

        Ok(RetrieveEntitiesResponse { entities })
    }

    fn map_row_to_entity(row: &SqliteRow, schemas: &[Ty]) -> Result<proto::types::Entity, Error> {
        let id =
            FieldElement::from_str(&row.get::<String, _>("id")).map_err(ParseError::FromStr)?;
        let models = schemas
            .iter()
            .map(|schema| {
                let mut struct_ty = schema.as_struct().expect("schema should be struct").to_owned();
                map_row_to_ty(&schema.name(), &mut struct_ty, row)?;

                Ok(struct_ty.try_into().unwrap())
            })
            .collect::<Result<Vec<_>, Error>>()?;

        Ok(proto::types::Entity { id: id.to_bytes_be().to_vec(), models })
    }
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
        let SubscribeEntitiesRequest { ids } = request.into_inner();
        let ids = ids
            .iter()
            .map(|id| {
                FieldElement::from_byte_slice_be(id)
                    .map_err(|e| Status::invalid_argument(e.to_string()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let rx = self.subscribe_entities(ids).await.map_err(|e| Status::internal(e.to_string()))?;

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
