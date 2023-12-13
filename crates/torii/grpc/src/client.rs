//! Client implementation for the gRPC service.
use std::num::ParseIntError;

use futures_util::stream::MapOk;
use futures_util::{Stream, StreamExt, TryStreamExt};
use starknet::core::types::{FromByteSliceError, FromStrError, StateUpdate};
use starknet_crypto::FieldElement;

use crate::proto::world::{
    world_client, MetadataRequest, RetrieveEntitiesRequest, RetrieveEntitiesResponse,
    SubscribeEntitiesRequest, SubscribeEntityResponse, SubscribeModelsRequest,
    SubscribeModelsResponse,
};
use crate::types::schema::Entity;
use crate::types::{KeysClause, Query};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Grpc(tonic::Status),
    #[error("Missing expected data")]
    MissingExpectedData,
    #[error("Unsupported type")]
    UnsupportedType,
    #[error(transparent)]
    ParseStr(FromStrError),
    #[error(transparent)]
    SliceError(FromByteSliceError),
    #[error(transparent)]
    ParseInt(ParseIntError),

    #[cfg(not(target_arch = "wasm32"))]
    #[error(transparent)]
    Transport(tonic::transport::Error),
}

/// A lightweight wrapper around the grpc client.
pub struct WorldClient {
    _world_address: FieldElement,
    #[cfg(not(target_arch = "wasm32"))]
    inner: world_client::WorldClient<tonic::transport::Channel>,
    #[cfg(target_arch = "wasm32")]
    inner: world_client::WorldClient<tonic_web_wasm_client::Client>,
}

impl WorldClient {
    #[cfg(not(target_arch = "wasm32"))]
    pub async fn new<D>(dst: D, _world_address: FieldElement) -> Result<Self, Error>
    where
        D: TryInto<tonic::transport::Endpoint>,
        D::Error: Into<Box<(dyn std::error::Error + Send + Sync + 'static)>>,
    {
        Ok(Self {
            _world_address,
            inner: world_client::WorldClient::connect(dst).await.map_err(Error::Transport)?,
        })
    }

    // we make this function async so that we can keep the function signature similar
    #[cfg(target_arch = "wasm32")]
    pub async fn new(endpoint: String, _world_address: FieldElement) -> Result<Self, Error> {
        Ok(Self {
            _world_address,
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
            .and_then(|metadata| metadata.try_into().map_err(Error::ParseStr))
    }

    pub async fn retrieve_entities(
        &mut self,
        query: Query,
    ) -> Result<RetrieveEntitiesResponse, Error> {
        let request = RetrieveEntitiesRequest { query: Some(query.into()) };
        self.inner.retrieve_entities(request).await.map_err(Error::Grpc).map(|res| res.into_inner())
    }

    /// Subscribe to entities updates of a World.
    pub async fn subscribe_entities(
        &mut self,
        ids: Vec<FieldElement>,
    ) -> Result<EntityUpdateStreaming, Error> {
        let ids = ids.iter().map(|id| id.to_bytes_be().to_vec()).collect();
        let stream = self
            .inner
            .subscribe_entities(SubscribeEntitiesRequest { ids })
            .await
            .map_err(Error::Grpc)
            .map(|res| res.into_inner())?;

        Ok(EntityUpdateStreaming(stream.map_ok(Box::new(|res| {
            let entity = res.entity.expect("entity must exist");
            entity.try_into().expect("must able to serialize")
        }))))
    }

    /// Subscribe to the model diff for a set of models of a World.
    pub async fn subscribe_model_diffs(
        &mut self,
        models_keys: Vec<KeysClause>,
    ) -> Result<ModelDiffsStreaming, Error> {
        let stream = self
            .inner
            .subscribe_models(SubscribeModelsRequest {
                models_keys: models_keys.into_iter().map(|e| e.into()).collect(),
            })
            .await
            .map_err(Error::Grpc)
            .map(|res| res.into_inner())?;

        Ok(ModelDiffsStreaming(stream.map_ok(Box::new(|res| {
            let update = res.model_update.expect("qed; state update must exist");
            TryInto::<StateUpdate>::try_into(update).expect("must able to serialize")
        }))))
    }
}

type ModelDiffMappedStream = MapOk<
    tonic::Streaming<SubscribeModelsResponse>,
    Box<dyn Fn(SubscribeModelsResponse) -> StateUpdate + Send>,
>;

pub struct ModelDiffsStreaming(ModelDiffMappedStream);

impl Stream for ModelDiffsStreaming {
    type Item = <ModelDiffMappedStream as Stream>::Item;
    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        self.0.poll_next_unpin(cx)
    }
}

type EntityMappedStream = MapOk<
    tonic::Streaming<SubscribeEntityResponse>,
    Box<dyn Fn(SubscribeEntityResponse) -> Entity + Send>,
>;

pub struct EntityUpdateStreaming(EntityMappedStream);

impl Stream for EntityUpdateStreaming {
    type Item = <EntityMappedStream as Stream>::Item;
    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        self.0.poll_next_unpin(cx)
    }
}
