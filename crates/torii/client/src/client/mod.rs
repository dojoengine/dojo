pub mod error;

use std::sync::Arc;

use dojo_types::WorldMetadata;
use dojo_world::contracts::WorldContractReader;
use futures::lock::Mutex;
use parking_lot::{RwLock, RwLockReadGuard};
use starknet::core::types::Felt;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::JsonRpcClient;
use tokio::sync::RwLock as AsyncRwLock;
use torii_grpc::client::{
    EntityUpdateStreaming, EventUpdateStreaming, IndexerUpdateStreaming, TokenBalanceStreaming, TokenUpdateStreaming,
};
use torii_grpc::proto::world::{
    RetrieveControllersResponse, RetrieveEntitiesResponse, RetrieveEventsResponse,
    RetrieveTokenBalancesResponse, RetrieveTokensResponse,
};
use torii_grpc::types::schema::Entity;
use torii_grpc::types::{
    Controller, EntityKeysClause, Event, EventQuery, Query, Token, TokenBalance,
};
use torii_relay::client::EventLoop;
use torii_relay::types::Message;

use crate::client::error::{Error, ParseError};

// TODO: remove reliance on RPC
#[allow(unused)]
#[derive(Debug)]
pub struct Client {
    /// Metadata of the World that the client is connected to.
    metadata: Arc<RwLock<WorldMetadata>>,
    /// The grpc client.
    inner: AsyncRwLock<torii_grpc::client::WorldClient>,
    /// Relay client.
    relay_client: torii_relay::client::RelayClient,
    /// The subscription client handle.
    /// World contract reader.
    world_reader: WorldContractReader<JsonRpcClient<HttpTransport>>,
}

impl Client {
    /// Returns a initialized [Client].
    pub async fn new(
        torii_url: String,
        rpc_url: String,
        relay_url: String,
        world: Felt,
    ) -> Result<Self, Error> {
        let mut grpc_client = torii_grpc::client::WorldClient::new(torii_url, world).await?;

        let relay_client = torii_relay::client::RelayClient::new(relay_url)?;

        let metadata = grpc_client.metadata().await?;

        let shared_metadata: Arc<_> = RwLock::new(metadata).into();

        // initialize the entities to be synced with the latest values
        let rpc_url = url::Url::parse(&rpc_url).map_err(ParseError::Url)?;
        let provider = JsonRpcClient::new(HttpTransport::new(rpc_url));
        let world_reader = WorldContractReader::new(world, provider);

        Ok(Self {
            world_reader,
            metadata: shared_metadata,
            inner: AsyncRwLock::new(grpc_client),
            relay_client,
        })
    }

    /// Starts the relay client event loop.
    /// This is a blocking call. Spawn this on a separate task.
    pub fn relay_runner(&self) -> Arc<Mutex<EventLoop>> {
        self.relay_client.event_loop.clone()
    }

    /// Publishes a message to a topic.
    /// Returns the message id.
    pub async fn publish_message(&self, message: Message) -> Result<Vec<u8>, Error> {
        self.relay_client
            .command_sender
            .publish(message)
            .await
            .map_err(Error::RelayClient)
            .map(|m| m.0)
    }

    /// Returns a read lock on the World metadata that the client is connected to.
    pub fn metadata(&self) -> RwLockReadGuard<'_, WorldMetadata> {
        self.metadata.read()
    }

    /// Retrieves controllers matching contract addresses.
    pub async fn controllers(
        &self,
        contract_addresses: Vec<Felt>,
    ) -> Result<Vec<Controller>, Error> {
        let mut grpc_client = self.inner.write().await;
        let RetrieveControllersResponse { controllers } =
            grpc_client.retrieve_controllers(contract_addresses).await?;
        Ok(controllers
            .into_iter()
            .map(TryInto::try_into)
            .collect::<Result<Vec<Controller>, _>>()?)
    }

    /// Retrieves tokens matching contract addresses.
    pub async fn tokens(&self, contract_addresses: Vec<Felt>) -> Result<Vec<Token>, Error> {
        let mut grpc_client = self.inner.write().await;
        let RetrieveTokensResponse { tokens } =
            grpc_client.retrieve_tokens(contract_addresses).await?;
        Ok(tokens.into_iter().map(TryInto::try_into).collect::<Result<Vec<Token>, _>>()?)
    }

    /// A direct stream to grpc subscribe tokens
    pub async fn on_token_updated(
        &self,
        contract_addresses: Vec<Felt>,
    ) -> Result<TokenUpdateStreaming, Error> {
        let mut grpc_client = self.inner.write().await;
        let stream = grpc_client.subscribe_tokens(contract_addresses).await?;
        Ok(stream)
    }

    /// Retrieves token balances for account addresses and contract addresses.
    pub async fn token_balances(
        &self,
        account_addresses: Vec<Felt>,
        contract_addresses: Vec<Felt>,
    ) -> Result<Vec<TokenBalance>, Error> {
        let mut grpc_client = self.inner.write().await;
        let RetrieveTokenBalancesResponse { balances } =
            grpc_client.retrieve_token_balances(account_addresses, contract_addresses).await?;
        Ok(balances.into_iter().map(TryInto::try_into).collect::<Result<Vec<TokenBalance>, _>>()?)
    }

    /// Retrieves entities matching query parameter.
    ///
    /// The query param includes an optional clause for filtering. Without clause, it fetches ALL
    /// entities, this is less efficient as it requires an additional query for each entity's
    /// model data. Specifying a clause can optimize the query by limiting the retrieval to specific
    /// type of entites matching keys and/or models.
    pub async fn entities(&self, query: Query) -> Result<Vec<Entity>, Error> {
        let mut grpc_client = self.inner.write().await;
        let RetrieveEntitiesResponse { entities, total_count: _ } =
            grpc_client.retrieve_entities(query).await?;
        Ok(entities.into_iter().map(TryInto::try_into).collect::<Result<Vec<Entity>, _>>()?)
    }

    /// Similary to entities, this function retrieves event messages matching the query parameter.
    pub async fn event_messages(
        &self,
        query: Query,
        historical: bool,
    ) -> Result<Vec<Entity>, Error> {
        let mut grpc_client = self.inner.write().await;
        let RetrieveEntitiesResponse { entities, total_count: _ } =
            grpc_client.retrieve_event_messages(query, historical).await?;
        Ok(entities.into_iter().map(TryInto::try_into).collect::<Result<Vec<Entity>, _>>()?)
    }

    /// Retrieve raw starknet events matching the keys provided.
    /// If the keys are empty, it will return all events.
    pub async fn starknet_events(&self, query: EventQuery) -> Result<Vec<Event>, Error> {
        let mut grpc_client = self.inner.write().await;
        let RetrieveEventsResponse { events } = grpc_client.retrieve_events(query).await?;
        Ok(events.into_iter().map(Event::from).collect::<Vec<Event>>())
    }

    /// A direct stream to grpc subscribe entities
    pub async fn on_entity_updated(
        &self,
        clauses: Vec<EntityKeysClause>,
    ) -> Result<EntityUpdateStreaming, Error> {
        let mut grpc_client = self.inner.write().await;
        let stream = grpc_client.subscribe_entities(clauses).await?;
        Ok(stream)
    }

    /// Update the entities subscription
    pub async fn update_entity_subscription(
        &self,
        subscription_id: u64,
        clauses: Vec<EntityKeysClause>,
    ) -> Result<(), Error> {
        let mut grpc_client = self.inner.write().await;
        grpc_client.update_entities_subscription(subscription_id, clauses).await?;
        Ok(())
    }

    /// A direct stream to grpc subscribe event messages
    pub async fn on_event_message_updated(
        &self,
        clauses: Vec<EntityKeysClause>,
        historical: bool,
    ) -> Result<EntityUpdateStreaming, Error> {
        let mut grpc_client = self.inner.write().await;
        let stream = grpc_client.subscribe_event_messages(clauses, historical).await?;
        Ok(stream)
    }

    /// Update the event messages subscription
    pub async fn update_event_message_subscription(
        &self,
        subscription_id: u64,
        clauses: Vec<EntityKeysClause>,
        historical: bool,
    ) -> Result<(), Error> {
        let mut grpc_client = self.inner.write().await;
        grpc_client
            .update_event_messages_subscription(subscription_id, clauses, historical)
            .await?;
        Ok(())
    }

    /// A direct stream to grpc subscribe starknet events
    pub async fn on_starknet_event(
        &self,
        keys: Vec<EntityKeysClause>,
    ) -> Result<EventUpdateStreaming, Error> {
        let mut grpc_client = self.inner.write().await;
        let stream = grpc_client.subscribe_events(keys).await?;
        Ok(stream)
    }

    /// Subscribe to indexer updates for a specific contract address.
    /// If no contract address is provided, it will subscribe to updates for world contract.
    pub async fn on_indexer_updated(
        &self,
        contract_address: Option<Felt>,
    ) -> Result<IndexerUpdateStreaming, Error> {
        let mut grpc_client = self.inner.write().await;
        let stream = grpc_client
            .subscribe_indexer(contract_address.unwrap_or(self.world_reader.address))
            .await?;
        Ok(stream)
    }

    /// Subscribes to token balances updates.
    /// If no contract addresses are provided, it will subscribe to updates for all contract
    /// addresses. If no account addresses are provided, it will subscribe to updates for all
    /// account addresses.
    pub async fn on_token_balance_updated(
        &self,
        contract_addresses: Vec<Felt>,
        account_addresses: Vec<Felt>,
    ) -> Result<TokenBalanceStreaming, Error> {
        let mut grpc_client = self.inner.write().await;
        let stream =
            grpc_client.subscribe_token_balances(contract_addresses, account_addresses).await?;
        Ok(stream)
    }

    /// Update the token balances subscription
    pub async fn update_token_balance_subscription(
        &self,
        subscription_id: u64,
        contract_addresses: Vec<Felt>,
        account_addresses: Vec<Felt>,
    ) -> Result<(), Error> {
        let mut grpc_client = self.inner.write().await;
        grpc_client
            .update_token_balances_subscription(
                subscription_id,
                contract_addresses,
                account_addresses,
            )
            .await?;
        Ok(())
    }
}
