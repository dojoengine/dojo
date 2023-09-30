pub mod error;
pub mod logger;
pub mod subscription;

use std::pin::Pin;
use std::str::FromStr;
use std::sync::Arc;

use futures::Stream;
use protos::world::{
    MetadataRequest, MetadataResponse, SubscribeEntitiesRequest, SubscribeEntitiesResponse,
};
use sqlx::{Executor, Pool, Row, Sqlite};
use starknet::core::types::FromStrError;
use starknet::core::utils::cairo_short_string_to_felt;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::JsonRpcClient;
use starknet_crypto::{poseidon_hash_many, FieldElement};
use tokio::sync::mpsc::{Receiver, Sender};
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};

use self::error::Error;
use self::subscription::{EntityModelRequest, EntitySubscriptionService};
use crate::protos::types::EntityModel;
use crate::protos::world::{GetEntityRequest, GetEntityResponse};
use crate::protos::{self};

#[derive(Debug, Clone)]
pub struct DojoWorld {
    world_address: FieldElement,
    pool: Pool<Sqlite>,
    /// Sender<(subscription requests, oneshot sender to send back the response)>
    subscription_req_sender:
        Sender<(EntityModelRequest, Sender<Result<SubscribeEntitiesResponse, Status>>)>,
}

impl DojoWorld {
    pub fn new(
        pool: Pool<Sqlite>,
        block_rx: Receiver<u64>,
        world_address: FieldElement,
        provider: Arc<JsonRpcClient<HttpTransport>>,
    ) -> Self {
        let (subscription_req_sender, rx) = tokio::sync::mpsc::channel(1);
        // spawn thread for state update service
        tokio::task::spawn(EntitySubscriptionService::new(provider, rx, block_rx));
        Self { pool, subscription_req_sender, world_address }
    }
}

impl DojoWorld {
    pub async fn metadata(&self) -> Result<protos::types::WorldMetadata, Error> {
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

        let models = sqlx::query_as(
            "SELECT c.name, c.class_hash, COUNT(cm.id) FROM models c LEFT JOIN model_members cm \
             ON c.id = cm.model_id GROUP BY c.id",
        )
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|(name, class_hash, size)| protos::types::ModelMetadata { name, class_hash, size })
        .collect::<Vec<_>>();

        let systems = sqlx::query_as("SELECT name, class_hash FROM systems")
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .map(|(name, class_hash)| protos::types::SystemMetadata { name, class_hash })
            .collect::<Vec<_>>();

        Ok(protos::types::WorldMetadata {
            systems,
            models,
            world_address,
            world_class_hash,
            executor_address,
            executor_class_hash,
        })
    }

    #[allow(unused)]
    pub async fn model_metadata(
        &self,
        component: String,
    ) -> Result<protos::types::ModelMetadata, Error> {
        sqlx::query_as(
            "SELECT c.name, c.class_hash, COUNT(cm.id) FROM models c LEFT JOIN model_members cm \
             ON c.id = cm.model_id WHERE c.id = ? GROUP BY c.id",
        )
        .bind(component.to_lowercase())
        .fetch_one(&self.pool)
        .await
        .map(|(name, class_hash, size)| protos::types::ModelMetadata { name, size, class_hash })
        .map_err(Error::from)
    }

    #[allow(unused)]
    pub async fn system_metadata(
        &self,
        system: String,
    ) -> Result<protos::types::SystemMetadata, Error> {
        sqlx::query_as("SELECT name, class_hash FROM systems WHERE id = ?")
            .bind(system.to_lowercase())
            .fetch_one(&self.pool)
            .await
            .map(|(name, class_hash)| protos::types::SystemMetadata { name, class_hash })
            .map_err(Error::from)
    }

    #[allow(unused)]
    async fn entity(
        &self,
        component: String,
        entity_keys: Vec<FieldElement>,
    ) -> Result<Vec<String>, Error> {
        let entity_id = format!("{:#x}", poseidon_hash_many(&entity_keys));
        // TODO: there's definitely a better way for doing this
        self.pool
            .fetch_one(
                format!(
                    "SELECT * FROM external_{} WHERE entity_id = '{entity_id}'",
                    component.to_lowercase()
                )
                .as_ref(),
            )
            .await
            .map_err(Error::from)
            .map(|row| {
                let size = row.columns().len() - 2;
                let mut values = Vec::with_capacity(size);
                for (i, _) in row.columns().iter().enumerate().skip(1).take(size) {
                    let value = match row.try_get::<String, _>(i) {
                        Ok(value) => value,
                        Err(sqlx::Error::ColumnDecode { .. }) => {
                            row.try_get::<u32, _>(i).expect("decode failed").to_string()
                        }
                        Err(e) => panic!("{e}"),
                    };
                    values.push(value);
                }
                values
            })
    }
}

type ServiceResult<T> = Result<Response<T>, Status>;
type SubscribeEntitiesResponseStream =
    Pin<Box<dyn Stream<Item = Result<SubscribeEntitiesResponse, Status>> + Send>>;

#[tonic::async_trait]
impl protos::world::world_server::World for DojoWorld {
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

    async fn get_entity(
        &self,
        request: Request<GetEntityRequest>,
    ) -> Result<Response<GetEntityResponse>, Status> {
        let GetEntityRequest { entity } = request.into_inner();

        let Some(EntityModel { model, keys }) = entity else {
            return Err(Status::invalid_argument("Entity not specified"));
        };

        let entity_keys = keys
            .iter()
            .map(|k| FieldElement::from_str(k))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| Status::invalid_argument(format!("Invalid key: {e}")))?;

        let values = self.entity(model, entity_keys).await.map_err(|e| match e {
            Error::Sql(sqlx::Error::RowNotFound) => Status::not_found("Entity not found"),
            e => Status::internal(e.to_string()),
        })?;

        Ok(Response::new(GetEntityResponse { values }))
    }

    type SubscribeEntitiesStream = SubscribeEntitiesResponseStream;

    async fn subscribe_entities(
        &self,
        request: Request<SubscribeEntitiesRequest>,
    ) -> ServiceResult<Self::SubscribeEntitiesStream> {
        let SubscribeEntitiesRequest { entities: raw_entities, world } = request.into_inner();
        let (sender, rx) = tokio::sync::mpsc::channel(128);

        let world = FieldElement::from_str(&world)
            .map_err(|e| Status::internal(format!("Invalid world address: {e}")))?;

        // in order to be able to compute all the storage address for all the requested entities, we
        // need to know the size of the entity component. we can get this information from the
        // sql database by querying the component metadata.

        let mut entities = Vec::with_capacity(raw_entities.len());
        for entity in raw_entities {
            let keys = entity
                .keys
                .into_iter()
                .map(|v| FieldElement::from_str(&v))
                .collect::<Result<Vec<FieldElement>, FromStrError>>()
                .map_err(|e| Status::internal(format!("parsing error: {e}")))?;

            let model = cairo_short_string_to_felt(&entity.model)
                .map_err(|e| Status::internal(format!("parsing error: {e}")))?;

            let (component_len,): (i64,) =
                sqlx::query_as("SELECT COUNT(*) FROM model_members WHERE model_id = ?")
                    .bind(entity.model.to_lowercase())
                    .fetch_one(&self.pool)
                    .await
                    .map_err(|e| match e {
                        sqlx::Error::RowNotFound => Status::not_found("Model not found"),
                        e => Status::internal(e.to_string()),
                    })?;

            entities.push(self::subscription::Entity {
                model: self::subscription::ModelMetadata {
                    name: model,
                    len: component_len as usize,
                },
                keys,
            })
        }

        self.subscription_req_sender
            .send((EntityModelRequest { world, entities }, sender))
            .await
            .expect("should send subscriber request");

        Ok(Response::new(Box::pin(ReceiverStream::new(rx)) as Self::SubscribeEntitiesStream))
    }
}
