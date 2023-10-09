pub mod error;
pub mod logger;
pub mod subscription;
pub mod utils;

use std::pin::Pin;
use std::str::FromStr;
use std::sync::Arc;

use futures::Stream;
use protos::world::{
    MetadataRequest, MetadataResponse, SubscribeEntitiesRequest, SubscribeEntitiesResponse,
};
use sqlx::{Pool, Sqlite};
use starknet::core::types::FromStrError;
use starknet::core::utils::cairo_short_string_to_felt;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::JsonRpcClient;
use starknet_crypto::FieldElement;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};

use self::error::Error;
use self::subscription::{EntityModelRequest, EntitySubscriptionService};
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

        let models =
            sqlx::query_as("SELECT name, class_hash, packed_size, unpacked_size FROM models")
                .fetch_all(&self.pool)
                .await?
                .into_iter()
                .map(|(name, class_hash, packed_size, unpacked_size)| {
                    protos::types::ModelMetadata { name, class_hash, packed_size, unpacked_size }
                })
                .collect::<Vec<_>>();

        Ok(protos::types::WorldMetadata {
            models,
            world_address,
            world_class_hash,
            executor_address,
            executor_class_hash,
        })
    }

    pub async fn model_metadata(
        &self,
        component: String,
    ) -> Result<protos::types::ModelMetadata, Error> {
        sqlx::query_as(
            "SELECT name, class_hash, packed_size, unpacked_size FROM models WHERE id = ?",
        )
        .bind(component)
        .fetch_one(&self.pool)
        .await
        .map(|(name, class_hash, packed_size, unpacked_size)| protos::types::ModelMetadata {
            name,
            class_hash,
            packed_size,
            unpacked_size,
        })
        .map_err(Error::from)
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

            let protos::types::ModelMetadata { packed_size, .. } = self
                .model_metadata(entity.model)
                .await
                .map_err(|e| Status::internal(e.to_string()))?;

            entities.push(self::subscription::Entity {
                keys,
                model: self::subscription::ModelMetadata {
                    name: model,
                    packed_size: packed_size as usize,
                },
            })
        }

        self.subscription_req_sender
            .send((EntityModelRequest { world, entities }, sender))
            .await
            .expect("should send subscriber request");

        Ok(Response::new(Box::pin(ReceiverStream::new(rx)) as Self::SubscribeEntitiesStream))
    }
}
