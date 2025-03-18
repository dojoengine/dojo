//! Client implementation for the gRPC service.
use std::num::ParseIntError;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Duration;

use crypto_bigint::{Encoding, U256};
use futures_util::stream::MapOk;
use futures_util::{Stream, StreamExt, TryStreamExt};
use starknet::core::types::{Felt, FromStrError, StateDiff, StateUpdate};
use tonic::codec::CompressionEncoding;
#[cfg(not(target_arch = "wasm32"))]
use tonic::transport::Endpoint;

use crate::proto::world::{
    world_client, RetrieveControllersRequest, RetrieveControllersResponse, RetrieveEntitiesRequest,
    RetrieveEntitiesResponse, RetrieveEventMessagesRequest, RetrieveEventsRequest,
    RetrieveEventsResponse, RetrieveTokenBalancesRequest, RetrieveTokenBalancesResponse,
    RetrieveTokensRequest, RetrieveTokensResponse, SubscribeEntitiesRequest,
    SubscribeEntityResponse, SubscribeEventMessagesRequest, SubscribeEventsRequest,
    SubscribeEventsResponse, SubscribeIndexerRequest, SubscribeIndexerResponse,
    SubscribeModelsRequest, SubscribeModelsResponse, SubscribeTokenBalancesResponse,
    SubscribeTokensResponse, UpdateEntitiesSubscriptionRequest,
    UpdateEventMessagesSubscriptionRequest, UpdateTokenBalancesSubscriptionRequest,
    UpdateTokenSubscriptionRequest, WorldMetadataRequest,
};
use crate::types::schema::{Entity, SchemaError};
use crate::types::{
    EntityKeysClause, Event, EventQuery, IndexerUpdate, ModelKeysClause, Query, Token, TokenBalance,
};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[cfg(not(target_arch = "wasm32"))]
    #[error("Endpoint error: {0}")]
    Endpoint(String),
    #[error(transparent)]
    Grpc(tonic::Status),
    #[error(transparent)]
    ParseStr(FromStrError),
    #[error(transparent)]
    ParseInt(ParseIntError),
    #[cfg(not(target_arch = "wasm32"))]
    #[error(transparent)]
    Transport(tonic::transport::Error),
    #[error(transparent)]
    Schema(#[from] SchemaError),
}

#[derive(Debug)]
/// A lightweight wrapper around the grpc client.
pub struct WorldClient {
    _world_address: Felt,
    #[cfg(not(target_arch = "wasm32"))]
    inner: world_client::WorldClient<tonic::transport::Channel>,
    #[cfg(target_arch = "wasm32")]
    inner: world_client::WorldClient<tonic_web_wasm_client::Client>,
}

impl WorldClient {
    #[cfg(not(target_arch = "wasm32"))]
    pub async fn new(dst: String, world_address: Felt) -> Result<Self, Error> {
        const KEEPALIVE_TIME: u64 = 60;

        let endpoint = Endpoint::from_shared(dst.clone())
            .map_err(|e| Error::Endpoint(e.to_string()))?
            .tcp_keepalive(Some(Duration::from_secs(KEEPALIVE_TIME)));
        let channel = endpoint.connect().await.map_err(Error::Transport)?;
        Ok(Self {
            _world_address: world_address,
            inner: world_client::WorldClient::with_origin(channel, endpoint.uri().clone())
                .accept_compressed(CompressionEncoding::Gzip)
                .send_compressed(CompressionEncoding::Gzip),
        })
    }

    // we make this function async so that we can keep the function signature similar
    #[cfg(target_arch = "wasm32")]
    pub async fn new(endpoint: String, _world_address: Felt) -> Result<Self, Error> {
        Ok(Self {
            _world_address,
            inner: world_client::WorldClient::new(tonic_web_wasm_client::Client::new(endpoint))
                .accept_compressed(CompressionEncoding::Gzip)
                .send_compressed(CompressionEncoding::Gzip),
        })
    }

    /// Retrieve the metadata of the World.
    pub async fn metadata(&mut self) -> Result<dojo_types::WorldMetadata, Error> {
        self.inner
            .world_metadata(WorldMetadataRequest {})
            .await
            .map_err(Error::Grpc)
            .and_then(|res| {
                res.into_inner()
                    .metadata
                    .ok_or(Error::Schema(SchemaError::MissingExpectedData("metadata".to_string())))
            })
            .and_then(|metadata| metadata.try_into().map_err(Error::ParseStr))
    }

    pub async fn retrieve_controllers(
        &mut self,
        contract_addresses: Vec<Felt>,
    ) -> Result<RetrieveControllersResponse, Error> {
        self.inner
            .retrieve_controllers(RetrieveControllersRequest {
                contract_addresses: contract_addresses
                    .into_iter()
                    .map(|c| c.to_bytes_be().to_vec())
                    .collect(),
            })
            .await
            .map_err(Error::Grpc)
            .map(|res| res.into_inner())
    }

    pub async fn retrieve_tokens(
        &mut self,
        contract_addresses: Vec<Felt>,
        token_ids: Vec<U256>,
    ) -> Result<RetrieveTokensResponse, Error> {
        self.inner
            .retrieve_tokens(RetrieveTokensRequest {
                contract_addresses: contract_addresses
                    .into_iter()
                    .map(|c| c.to_bytes_be().to_vec())
                    .collect(),
                token_ids: token_ids.into_iter().map(|id| id.to_be_bytes().to_vec()).collect(),
            })
            .await
            .map_err(Error::Grpc)
            .map(|res| res.into_inner())
    }

    pub async fn subscribe_tokens(
        &mut self,
        contract_addresses: Vec<Felt>,
        token_ids: Vec<U256>,
    ) -> Result<TokenUpdateStreaming, Error> {
        let request = RetrieveTokensRequest {
            contract_addresses: contract_addresses
                .into_iter()
                .map(|c| c.to_bytes_be().to_vec())
                .collect(),
            token_ids: token_ids.into_iter().map(|id| id.to_be_bytes().to_vec()).collect(),
        };
        let stream = self
            .inner
            .subscribe_tokens(request)
            .await
            .map_err(Error::Grpc)
            .map(|res| res.into_inner())?;
        Ok(TokenUpdateStreaming(stream.map_ok(Box::new(|res| {
            (
                res.subscription_id,
                match res.token {
                    Some(token) => token.try_into().expect("must able to serialize"),
                    None => Token {
                        token_id: U256::ZERO,
                        contract_address: Felt::ZERO,
                        name: "".to_string(),
                        symbol: "".to_string(),
                        decimals: 0,
                        metadata: "".to_string(),
                    },
                },
            )
        }))))
    }

    pub async fn update_tokens_subscription(
        &mut self,
        subscription_id: u64,
        contract_addresses: Vec<Felt>,
        token_ids: Vec<U256>,
    ) -> Result<(), Error> {
        let contract_addresses =
            contract_addresses.into_iter().map(|c| c.to_bytes_be().to_vec()).collect();
        let request = UpdateTokenSubscriptionRequest {
            subscription_id,
            contract_addresses,
            token_ids: token_ids.into_iter().map(|id| id.to_be_bytes().to_vec()).collect(),
        };
        self.inner
            .update_tokens_subscription(request)
            .await
            .map_err(Error::Grpc)
            .map(|res| res.into_inner())
    }

    pub async fn retrieve_token_balances(
        &mut self,
        account_addresses: Vec<Felt>,
        contract_addresses: Vec<Felt>,
        token_ids: Vec<U256>,
    ) -> Result<RetrieveTokenBalancesResponse, Error> {
        self.inner
            .retrieve_token_balances(RetrieveTokenBalancesRequest {
                account_addresses: account_addresses
                    .into_iter()
                    .map(|a| a.to_bytes_be().to_vec())
                    .collect(),
                contract_addresses: contract_addresses
                    .into_iter()
                    .map(|c| c.to_bytes_be().to_vec())
                    .collect(),
                token_ids: token_ids.into_iter().map(|id| id.to_be_bytes().to_vec()).collect(),
            })
            .await
            .map_err(Error::Grpc)
            .map(|res| res.into_inner())
    }

    pub async fn retrieve_entities(
        &mut self,
        query: Query,
        historical: bool,
    ) -> Result<RetrieveEntitiesResponse, Error> {
        let request = RetrieveEntitiesRequest { query: Some(query.into()), historical };
        self.inner.retrieve_entities(request).await.map_err(Error::Grpc).map(|res| res.into_inner())
    }

    pub async fn retrieve_event_messages(
        &mut self,
        query: Query,
        historical: bool,
    ) -> Result<RetrieveEntitiesResponse, Error> {
        let request = RetrieveEventMessagesRequest { query: Some(query.into()), historical };
        self.inner
            .retrieve_event_messages(request)
            .await
            .map_err(Error::Grpc)
            .map(|res| res.into_inner())
    }

    pub async fn retrieve_events(
        &mut self,
        query: EventQuery,
    ) -> Result<RetrieveEventsResponse, Error> {
        let request = RetrieveEventsRequest { query: Some(query.into()) };
        self.inner.retrieve_events(request).await.map_err(Error::Grpc).map(|res| res.into_inner())
    }

    /// Subscribe to indexer updates.
    pub async fn subscribe_indexer(
        &mut self,
        contract_address: Felt,
    ) -> Result<IndexerUpdateStreaming, Error> {
        let request =
            SubscribeIndexerRequest { contract_address: contract_address.to_bytes_be().to_vec() };
        let stream = self
            .inner
            .subscribe_indexer(request)
            .await
            .map_err(Error::Grpc)
            .map(|res| res.into_inner())?;
        Ok(IndexerUpdateStreaming(stream.map_ok(Box::new(|res| res.into()))))
    }

    /// Subscribe to entities updates of a World.
    pub async fn subscribe_entities(
        &mut self,
        clauses: Vec<EntityKeysClause>,
    ) -> Result<EntityUpdateStreaming, Error> {
        let clauses = clauses.into_iter().map(|c| c.into()).collect();
        let stream = self
            .inner
            .subscribe_entities(SubscribeEntitiesRequest { clauses })
            .await
            .map_err(Error::Grpc)
            .map(|res| res.into_inner())?;

        Ok(EntityUpdateStreaming(stream.map_ok(Box::new(|res| {
            res.entity.map_or(
                (res.subscription_id, Entity { hashed_keys: Felt::ZERO, models: vec![] }),
                |entity| (res.subscription_id, entity.try_into().expect("must able to serialize")),
            )
        }))))
    }

    /// Update an entities subscription.
    pub async fn update_entities_subscription(
        &mut self,
        subscription_id: u64,
        clauses: Vec<EntityKeysClause>,
    ) -> Result<(), Error> {
        let clauses = clauses.into_iter().map(|c| c.into()).collect();

        self.inner
            .update_entities_subscription(UpdateEntitiesSubscriptionRequest {
                subscription_id,
                clauses,
            })
            .await
            .map_err(Error::Grpc)
            .map(|res| res.into_inner())
    }

    /// Subscribe to event messages of a World.
    pub async fn subscribe_event_messages(
        &mut self,
        clauses: Vec<EntityKeysClause>,
    ) -> Result<EntityUpdateStreaming, Error> {
        let clauses = clauses.into_iter().map(|c| c.into()).collect();
        let stream = self
            .inner
            .subscribe_event_messages(SubscribeEventMessagesRequest { clauses })
            .await
            .map_err(Error::Grpc)
            .map(|res| res.into_inner())?;

        Ok(EntityUpdateStreaming(stream.map_ok(Box::new(|res| {
            res.entity.map_or(
                (res.subscription_id, Entity { hashed_keys: Felt::ZERO, models: vec![] }),
                |entity| (res.subscription_id, entity.try_into().expect("must able to serialize")),
            )
        }))))
    }

    /// Update an event messages subscription.
    pub async fn update_event_messages_subscription(
        &mut self,
        subscription_id: u64,
        clauses: Vec<EntityKeysClause>,
    ) -> Result<(), Error> {
        let clauses = clauses.into_iter().map(|c| c.into()).collect();
        self.inner
            .update_event_messages_subscription(UpdateEventMessagesSubscriptionRequest {
                subscription_id,
                clauses,
            })
            .await
            .map_err(Error::Grpc)
            .map(|res| res.into_inner())
    }

    /// Subscribe to the events of a World.
    pub async fn subscribe_events(
        &mut self,
        keys: Vec<EntityKeysClause>,
    ) -> Result<EventUpdateStreaming, Error> {
        let keys = keys.into_iter().map(|c| c.into()).collect();

        let stream = self
            .inner
            .subscribe_events(SubscribeEventsRequest { keys })
            .await
            .map_err(Error::Grpc)
            .map(|res| res.into_inner())?;

        Ok(EventUpdateStreaming(stream.map_ok(Box::new(|res| match res.event {
            Some(event) => event.into(),
            None => Event { keys: vec![], data: vec![], transaction_hash: Felt::ZERO },
        }))))
    }

    /// Subscribe to the model diff for a set of models of a World.
    pub async fn subscribe_model_diffs(
        &mut self,
        models_keys: Vec<ModelKeysClause>,
    ) -> Result<ModelDiffsStreaming, Error> {
        let stream = self
            .inner
            .subscribe_models(SubscribeModelsRequest {
                models_keys: models_keys.into_iter().map(|e| e.into()).collect(),
            })
            .await
            .map_err(Error::Grpc)
            .map(|res| res.into_inner())?;

        Ok(ModelDiffsStreaming(stream.map_ok(Box::new(|res| match res.model_update {
            Some(update) => {
                TryInto::<StateUpdate>::try_into(update).expect("must able to serialize")
            }
            None => empty_state_update(),
        }))))
    }

    /// Subscribe to token balances.
    pub async fn subscribe_token_balances(
        &mut self,
        contract_addresses: Vec<Felt>,
        account_addresses: Vec<Felt>,
        token_ids: Vec<U256>,
    ) -> Result<TokenBalanceStreaming, Error> {
        let request = RetrieveTokenBalancesRequest {
            contract_addresses: contract_addresses
                .into_iter()
                .map(|c| c.to_bytes_be().to_vec())
                .collect(),
            account_addresses: account_addresses
                .into_iter()
                .map(|a| a.to_bytes_be().to_vec())
                .collect(),
            token_ids: token_ids.into_iter().map(|id| id.to_be_bytes().to_vec()).collect(),
        };
        let stream = self
            .inner
            .subscribe_token_balances(request)
            .await
            .map_err(Error::Grpc)
            .map(|res| res.into_inner())?;
        Ok(TokenBalanceStreaming(stream.map_ok(Box::new(|res| {
            (
                res.subscription_id,
                match res.balance {
                    Some(balance) => balance.try_into().expect("must able to serialize"),
                    None => TokenBalance {
                        balance: U256::ZERO,
                        account_address: Felt::ZERO,
                        contract_address: Felt::ZERO,
                        token_id: U256::ZERO,
                    },
                },
            )
        }))))
    }

    /// Update a token balances subscription.
    pub async fn update_token_balances_subscription(
        &mut self,
        subscription_id: u64,
        contract_addresses: Vec<Felt>,
        account_addresses: Vec<Felt>,
        token_ids: Vec<U256>,
    ) -> Result<(), Error> {
        let request = UpdateTokenBalancesSubscriptionRequest {
            subscription_id,
            contract_addresses: contract_addresses
                .into_iter()
                .map(|c| c.to_bytes_be().to_vec())
                .collect(),
            account_addresses: account_addresses
                .into_iter()
                .map(|a| a.to_bytes_be().to_vec())
                .collect(),
            token_ids: token_ids.into_iter().map(|id| id.to_be_bytes().to_vec()).collect(),
        };
        self.inner
            .update_token_balances_subscription(request)
            .await
            .map_err(Error::Grpc)
            .map(|res| res.into_inner())
    }
}

type TokenMappedStream = MapOk<
    tonic::Streaming<SubscribeTokensResponse>,
    Box<dyn Fn(SubscribeTokensResponse) -> (SubscriptionId, Token) + Send>,
>;

#[derive(Debug)]
pub struct TokenUpdateStreaming(TokenMappedStream);

impl Stream for TokenUpdateStreaming {
    type Item = <TokenMappedStream as Stream>::Item;
    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        self.0.poll_next_unpin(cx)
    }
}

type TokenBalanceMappedStream = MapOk<
    tonic::Streaming<SubscribeTokenBalancesResponse>,
    Box<dyn Fn(SubscribeTokenBalancesResponse) -> (SubscriptionId, TokenBalance) + Send>,
>;

#[derive(Debug)]
pub struct TokenBalanceStreaming(TokenBalanceMappedStream);

impl Stream for TokenBalanceStreaming {
    type Item = <TokenBalanceMappedStream as Stream>::Item;
    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        self.0.poll_next_unpin(cx)
    }
}

type ModelDiffMappedStream = MapOk<
    tonic::Streaming<SubscribeModelsResponse>,
    Box<dyn Fn(SubscribeModelsResponse) -> StateUpdate + Send>,
>;

#[derive(Debug)]
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

type SubscriptionId = u64;
type EntityMappedStream = MapOk<
    tonic::Streaming<SubscribeEntityResponse>,
    Box<dyn Fn(SubscribeEntityResponse) -> (SubscriptionId, Entity) + Send>,
>;

#[derive(Debug)]
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

type EventMappedStream = MapOk<
    tonic::Streaming<SubscribeEventsResponse>,
    Box<dyn Fn(SubscribeEventsResponse) -> Event + Send>,
>;

#[derive(Debug)]
pub struct EventUpdateStreaming(EventMappedStream);

impl Stream for EventUpdateStreaming {
    type Item = <EventMappedStream as Stream>::Item;
    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        self.0.poll_next_unpin(cx)
    }
}

type IndexerMappedStream = MapOk<
    tonic::Streaming<SubscribeIndexerResponse>,
    Box<dyn Fn(SubscribeIndexerResponse) -> IndexerUpdate + Send>,
>;

#[derive(Debug)]
pub struct IndexerUpdateStreaming(IndexerMappedStream);

impl Stream for IndexerUpdateStreaming {
    type Item = <IndexerMappedStream as Stream>::Item;
    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        self.0.poll_next_unpin(cx)
    }
}

fn empty_state_update() -> StateUpdate {
    StateUpdate {
        block_hash: Felt::ZERO,
        new_root: Felt::ZERO,
        old_root: Felt::ZERO,
        state_diff: StateDiff {
            declared_classes: vec![],
            deployed_contracts: vec![],
            deprecated_declared_classes: vec![],
            nonces: vec![],
            replaced_classes: vec![],
            storage_diffs: vec![],
        },
    }
}
