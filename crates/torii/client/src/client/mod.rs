pub mod error;
pub mod storage;
pub mod subscription;

use std::cell::OnceCell;
use std::collections::HashSet;
use std::sync::Arc;

use dojo_types::packing::unpack;
use dojo_types::schema::Ty;
use dojo_types::WorldMetadata;
use dojo_world::contracts::WorldContractReader;
use futures::channel::mpsc::UnboundedReceiver;
use futures_util::lock::Mutex;
use parking_lot::{RwLock, RwLockReadGuard};
use starknet::core::utils::cairo_short_string_to_felt;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::JsonRpcClient;
use starknet_crypto::FieldElement;
use tokio::sync::RwLock as AsyncRwLock;
use torii_grpc::client::{EntityUpdateStreaming, ModelDiffsStreaming};
use torii_grpc::proto::world::RetrieveEntitiesResponse;
use torii_grpc::types::schema::Entity;
use torii_grpc::types::{KeysClause, Query};
use torii_relay::client::{EventLoop, Message};

use crate::client::error::{Error, ParseError};
use crate::client::storage::ModelStorage;
use crate::client::subscription::{
    SubscribedModels, SubscriptionClientHandle, SubscriptionService,
};

// TODO: remove reliance on RPC
#[allow(unused)]
pub struct Client {
    /// Metadata of the World that the client is connected to.
    metadata: Arc<RwLock<WorldMetadata>>,
    /// The grpc client.
    inner: AsyncRwLock<torii_grpc::client::WorldClient>,
    /// Relay client.
    relay_client: torii_relay::client::RelayClient,
    /// Model storage
    storage: Arc<ModelStorage>,
    /// Models the client are subscribed to.
    subscribed_models: Arc<SubscribedModels>,
    /// The subscription client handle.
    sub_client_handle: OnceCell<SubscriptionClientHandle>,
    /// World contract reader.
    world_reader: WorldContractReader<JsonRpcClient<HttpTransport>>,
}

impl Client {
    /// Returns a initialized [Client].
    pub async fn new(
        torii_url: String,
        rpc_url: String,
        relay_url: String,
        world: FieldElement,
        models_keys: Option<Vec<KeysClause>>,
    ) -> Result<Self, Error> {
        let mut grpc_client = torii_grpc::client::WorldClient::new(torii_url, world).await?;

        let relay_client = torii_relay::client::RelayClient::new(relay_url)?;

        let metadata = grpc_client.metadata().await?;

        let shared_metadata: Arc<_> = RwLock::new(metadata).into();
        let client_storage: Arc<_> = ModelStorage::new(shared_metadata.clone()).into();
        let subbed_models: Arc<_> = SubscribedModels::new(shared_metadata.clone()).into();

        // initialize the entities to be synced with the latest values
        let rpc_url = url::Url::parse(&rpc_url).map_err(ParseError::Url)?;
        let provider = JsonRpcClient::new(HttpTransport::new(rpc_url));
        let world_reader = WorldContractReader::new(world, provider);

        if let Some(keys) = models_keys {
            subbed_models.add_models(keys)?;

            // TODO: change this to querying the gRPC url instead
            let subbed_models = subbed_models.models_keys.read().clone();
            for keys in subbed_models {
                let model_reader = world_reader.model_reader(&keys.model).await?;
                let values = model_reader.entity_storage(&keys.keys).await?;

                client_storage.set_model_storage(
                    cairo_short_string_to_felt(&keys.model).unwrap(),
                    keys.keys,
                    values,
                )?;
            }
        }

        Ok(Self {
            world_reader,
            storage: client_storage,
            metadata: shared_metadata,
            sub_client_handle: OnceCell::new(),
            inner: AsyncRwLock::new(grpc_client),
            relay_client,
            subscribed_models: subbed_models,
        })
    }

    /// Waits for the relay to be ready and listening for messages.
    pub async fn wait_for_relay(&mut self) -> Result<(), Error> {
        self.relay_client.command_sender.wait_for_relay().await.map_err(Error::RelayClient)
    }

    /// Subscribes to a topic.
    /// Returns true if the topic was subscribed to.
    /// Returns false if the topic was already subscribed to.
    pub async fn subscribe_topic(&mut self, topic: String) -> Result<bool, Error> {
        self.relay_client.command_sender.subscribe(topic).await.map_err(Error::RelayClient)
    }

    /// Unsubscribes from a topic.
    /// Returns true if the topic was subscribed to.
    pub async fn unsubscribe_topic(&mut self, topic: String) -> Result<bool, Error> {
        self.relay_client.command_sender.unsubscribe(topic).await.map_err(Error::RelayClient)
    }

    /// Publishes a message to a topic.
    /// Returns the message id.
    pub async fn publish_message(&mut self, topic: &str, message: &[u8]) -> Result<Vec<u8>, Error> {
        self.relay_client
            .command_sender
            .publish(topic.to_string(), message.to_vec())
            .await
            .map_err(Error::RelayClient)
            .map(|m| m.0)
    }

    /// Returns the event loop of the relay client.
    /// Which can then be used to run the relay client
    pub fn relay_client_runner(&self) -> Arc<Mutex<EventLoop>> {
        self.relay_client.event_loop.clone()
    }

    /// Returns the message receiver of the relay client.
    pub fn relay_client_stream(&self) -> Arc<Mutex<UnboundedReceiver<Message>>> {
        self.relay_client.message_receiver.clone()
    }

    /// Returns a read lock on the World metadata that the client is connected to.
    pub fn metadata(&self) -> RwLockReadGuard<'_, WorldMetadata> {
        self.metadata.read()
    }

    pub fn subscribed_models(&self) -> RwLockReadGuard<'_, HashSet<KeysClause>> {
        self.subscribed_models.models_keys.read()
    }

    /// Retrieves entities matching query parameter.
    ///
    /// The query param includes an optional clause for filtering. Without clause, it fetches ALL
    /// entities, this is less efficient as it requires an additional query for each entity's
    /// model data. Specifying a clause can optimize the query by limiting the retrieval to specific
    /// type of entites matching keys and/or models.
    pub async fn entities(&self, query: Query) -> Result<Vec<Entity>, Error> {
        let mut grpc_client = self.inner.write().await;
        let RetrieveEntitiesResponse { entities } = grpc_client.retrieve_entities(query).await?;
        Ok(entities.into_iter().map(TryInto::try_into).collect::<Result<Vec<Entity>, _>>()?)
    }

    /// A direct stream to grpc subscribe entities
    pub async fn on_entity_updated(
        &self,
        ids: Vec<FieldElement>,
    ) -> Result<EntityUpdateStreaming, Error> {
        let mut grpc_client = self.inner.write().await;
        let stream = grpc_client.subscribe_entities(ids).await?;
        Ok(stream)
    }

    /// Returns the value of a model.
    ///
    /// This function will only return `None`, if `model` doesn't exist. If there is no model with
    /// the specified `keys`, it will return a [`Ty`] with the default values.
    ///
    /// If the requested model is not among the synced models, it will attempt to fetch it from
    /// the RPC.
    pub async fn model(&self, keys: &KeysClause) -> Result<Option<Ty>, Error> {
        let Some(mut schema) = self.metadata.read().model(&keys.model).map(|m| m.schema.clone())
        else {
            return Ok(None);
        };

        if !self.subscribed_models.is_synced(keys) {
            let model = self.world_reader.model_reader(&keys.model).await?;
            return Ok(Some(model.entity(&keys.keys).await?));
        }

        let Ok(Some(raw_values)) = self.storage.get_model_storage(
            cairo_short_string_to_felt(&keys.model).map_err(ParseError::CairoShortStringToFelt)?,
            &keys.keys,
        ) else {
            return Ok(Some(schema));
        };

        let layout = self
            .metadata
            .read()
            .model(&keys.model)
            .map(|m| m.layout.clone())
            .expect("qed; layout should exist");

        let unpacked = unpack(raw_values, layout).unwrap();
        let mut keys_and_unpacked = [keys.keys.to_vec(), unpacked].concat();

        schema.deserialize(&mut keys_and_unpacked).unwrap();

        Ok(Some(schema))
    }

    /// Initiate the model subscriptions and returns a [SubscriptionService] which when await'ed
    /// will execute the subscription service and starts the syncing process.
    pub async fn start_subscription(&self) -> Result<SubscriptionService, Error> {
        let models_keys: Vec<KeysClause> =
            self.subscribed_models.models_keys.read().clone().into_iter().collect();
        let sub_res_stream = self.initiate_subscription(models_keys).await?;

        let (service, handle) = SubscriptionService::new(
            Arc::clone(&self.storage),
            Arc::clone(&self.metadata),
            Arc::clone(&self.subscribed_models),
            sub_res_stream,
        );

        self.sub_client_handle.set(handle).unwrap();
        Ok(service)
    }

    /// Adds entities to the list of entities to be synced.
    ///
    /// NOTE: This will establish a new subscription stream with the server.
    pub async fn add_models_to_sync(&self, models_keys: Vec<KeysClause>) -> Result<(), Error> {
        for keys in &models_keys {
            self.initiate_model(&keys.model, keys.keys.clone()).await?;
        }

        self.subscribed_models.add_models(models_keys)?;

        let updated_models =
            self.subscribed_models.models_keys.read().clone().into_iter().collect();
        let sub_res_stream = self.initiate_subscription(updated_models).await?;

        match self.sub_client_handle.get() {
            Some(handle) => handle.update_subscription_stream(sub_res_stream),
            None => return Err(Error::SubscriptionUninitialized),
        }
        Ok(())
    }

    /// Removes models from the list of models to be synced.
    ///
    /// NOTE: This will establish a new subscription stream with the server.
    pub async fn remove_models_to_sync(&self, models_keys: Vec<KeysClause>) -> Result<(), Error> {
        self.subscribed_models.remove_models(models_keys)?;

        let updated_entities =
            self.subscribed_models.models_keys.read().clone().into_iter().collect();
        let sub_res_stream = self.initiate_subscription(updated_entities).await?;

        match self.sub_client_handle.get() {
            Some(handle) => handle.update_subscription_stream(sub_res_stream),
            None => return Err(Error::SubscriptionUninitialized),
        }
        Ok(())
    }

    pub fn storage(&self) -> Arc<ModelStorage> {
        Arc::clone(&self.storage)
    }

    async fn initiate_subscription(
        &self,
        keys: Vec<KeysClause>,
    ) -> Result<ModelDiffsStreaming, Error> {
        let mut grpc_client = self.inner.write().await;
        let stream = grpc_client.subscribe_model_diffs(keys).await?;
        Ok(stream)
    }

    async fn initiate_model(&self, model: &str, keys: Vec<FieldElement>) -> Result<(), Error> {
        let model_reader = self.world_reader.model_reader(model).await?;
        let values = model_reader.entity_storage(&keys).await?;
        self.storage.set_model_storage(
            cairo_short_string_to_felt(model).map_err(ParseError::CairoShortStringToFelt)?,
            keys,
            values,
        )?;
        Ok(())
    }
}
