//! Client implementation for the gRPC service.

use std::str::FromStr;

use protos::world::{world_client, SubscribeEntitiesRequest};
use starknet::core::types::FromStrError;
use starknet_crypto::FieldElement;

use crate::protos::world::{GetEntityRequest, MetadataRequest, SubscribeEntitiesResponse};
use crate::protos::{self, types};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Grpc(tonic::Status),
    #[error("Missing expected data")]
    MissingExpectedData,
    #[error(transparent)]
    Parsing(FromStrError),

    #[cfg(not(target_arch = "wasm32"))]
    #[error(transparent)]
    Transport(tonic::transport::Error),
}

/// A lightweight wrapper around the grpc client.
pub struct WorldClient {
    world_address: FieldElement,
    #[cfg(not(target_arch = "wasm32"))]
    inner: world_client::WorldClient<tonic::transport::Channel>,
    #[cfg(target_arch = "wasm32")]
    inner: world_client::WorldClient<tonic_web_wasm_client::Client>,
}

impl WorldClient {
    #[cfg(not(target_arch = "wasm32"))]
    pub async fn new<D>(dst: D, world_address: FieldElement) -> Result<Self, Error>
    where
        D: TryInto<tonic::transport::Endpoint>,
        D::Error: Into<Box<(dyn std::error::Error + Send + Sync + 'static)>>,
    {
        Ok(Self {
            world_address,
            inner: world_client::WorldClient::connect(dst).await.map_err(Error::Transport)?,
        })
    }

    // we make this function async so that we can keep the function signature similar
    #[cfg(target_arch = "wasm32")]
    pub async fn new(endpoint: String, world_address: FieldElement) -> Result<Self, Error> {
        Ok(Self {
            world_address,
            inner: world_client::WorldClient::new(tonic_web_wasm_client::Client::new(endpoint)),
        })
    }

    /// Retrieve the metadata of the World.
    pub async fn metadata(&mut self) -> Result<dojo_types::WorldMetadata, Error> {
        self.inner
            .world_metadata(MetadataRequest {})
            .await
            .map_err(Error::Grpc)
            .and_then(|res| res.into_inner().metadata.ok_or(Error::MissingExpectedData))
            .and_then(|metadata| metadata.try_into().map_err(Error::Parsing))
    }

    /// Retrieves the latest model value of the requested entity keys
    pub async fn get_entity(
        &mut self,
        model: String,
        keys: Vec<FieldElement>,
    ) -> Result<Vec<FieldElement>, Error> {
        let values = self
            .inner
            .get_entity(GetEntityRequest {
                entity: Some(types::EntityModel {
                    model,
                    keys: keys.into_iter().map(|k| format!("{k:#x}")).collect(),
                }),
            })
            .await
            .map(|res| res.into_inner().values)
            .map_err(Error::Grpc)?;

        values
            .iter()
            .map(|v| FieldElement::from_str(v))
            .collect::<Result<Vec<_>, _>>()
            .map_err(Error::Parsing)
    }

    /// Subscribe to the state diff for a set of entities of a World.
    pub async fn subscribe_entities(
        &mut self,
        entities: Vec<dojo_types::schema::EntityModel>,
    ) -> Result<tonic::Streaming<SubscribeEntitiesResponse>, Error> {
        self.inner
            .subscribe_entities(SubscribeEntitiesRequest {
                entities: entities
                    .into_iter()
                    .map(|e| protos::types::EntityModel {
                        model: e.model,
                        keys: e.keys.into_iter().map(|felt| format!("{felt:#x}")).collect(),
                    })
                    .collect(),
                world: format!("{:#x}", self.world_address),
            })
            .await
            .map_err(Error::Grpc)
            .map(|res| res.into_inner())
    }
}
