pub mod error;
pub mod storage;
pub mod subscription;

use std::cell::OnceCell;
use std::collections::HashSet;
use std::sync::Arc;

use dojo_types::packing::unpack;
use dojo_types::schema::{EntityModel, Ty};
use dojo_types::WorldMetadata;
use dojo_world::contracts::WorldContractReader;
use parking_lot::{RwLock, RwLockReadGuard};
use starknet::core::utils::cairo_short_string_to_felt;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::JsonRpcClient;
use starknet_crypto::FieldElement;
use tokio::sync::RwLock as AsyncRwLock;
use torii_grpc::client::EntityUpdateStreaming;

use self::error::{Error, ParseError};
use self::storage::ModelStorage;
use self::subscription::{SubscribedEntities, SubscriptionClientHandle};
use crate::client::subscription::SubscriptionService;

// TODO: remove reliance on RPC
#[allow(unused)]
pub struct Client {
    /// Metadata of the World that the client is connected to.
    metadata: Arc<RwLock<WorldMetadata>>,
    /// The grpc client.
    inner: AsyncRwLock<torii_grpc::client::WorldClient>,
    /// Entity storage
    storage: Arc<ModelStorage>,
    /// Entities the client are subscribed to.
    subscribed_entities: Arc<SubscribedEntities>,
    /// The subscription client handle.
    sub_client_handle: OnceCell<SubscriptionClientHandle>,
    /// World contract reader.
    world_reader: WorldContractReader<JsonRpcClient<HttpTransport>>,
}

impl Client {
    /// Returns a [ClientBuilder] for building a [Client].
    pub fn build() -> ClientBuilder {
        ClientBuilder::new()
    }

    /// Returns a read lock on the World metadata that the client is connected to.
    pub fn metadata(&self) -> RwLockReadGuard<'_, WorldMetadata> {
        self.metadata.read()
    }

    pub fn subscribed_entities(&self) -> RwLockReadGuard<'_, HashSet<EntityModel>> {
        self.subscribed_entities.entities.read()
    }

    /// Returns the model value of an entity.
    ///
    /// This function will only return `None`, if `model` doesn't exist. If there is no entity with
    /// the specified `keys`, it will return a [`Ty`] with the default values.
    pub fn entity(&self, model: &str, keys: &[FieldElement]) -> Option<Ty> {
        let mut schema = self.metadata.read().model(model).map(|m| m.schema.clone())?;

        let Ok(Some(raw_values)) =
            self.storage.get_entity_storage(cairo_short_string_to_felt(model).ok()?, keys)
        else {
            return Some(schema);
        };

        let layout = self
            .metadata
            .read()
            .model(model)
            .map(|m| m.layout.clone())
            .expect("qed; layout should exist");

        let unpacked = unpack(raw_values, layout).unwrap();
        let mut keys_and_unpacked = [keys.to_vec(), unpacked].concat();

        schema.deserialize(&mut keys_and_unpacked).unwrap();

        Some(schema)
    }

    /// Initiate the entity subscriptions and returns a [SubscriptionService] which when await'ed
    /// will execute the subscription service and starts the syncing process.
    pub async fn start_subscription(&self) -> Result<SubscriptionService, Error> {
        let entities = self.subscribed_entities.entities.read().clone().into_iter().collect();
        let sub_res_stream = self.initiate_subscription(entities).await?;

        let (service, handle) = SubscriptionService::new(
            Arc::clone(&self.storage),
            Arc::clone(&self.metadata),
            Arc::clone(&self.subscribed_entities),
            sub_res_stream,
        );

        self.sub_client_handle.set(handle).unwrap();
        Ok(service)
    }

    /// Adds entities to the list of entities to be synced.
    ///
    /// NOTE: This will establish a new subscription stream with the server.
    pub async fn add_entities_to_sync(&self, entities: Vec<EntityModel>) -> Result<(), Error> {
        for entity in &entities {
            self.initiate_entity(&entity.model, entity.keys.clone()).await?;
        }

        self.subscribed_entities.add_entities(entities)?;

        let updated_entities =
            self.subscribed_entities.entities.read().clone().into_iter().collect();
        let sub_res_stream = self.initiate_subscription(updated_entities).await?;

        match self.sub_client_handle.get() {
            Some(handle) => handle.update_subscription_stream(sub_res_stream),
            None => return Err(Error::SubscriptionUninitialized),
        }
        Ok(())
    }

    /// Removes entities from the list of entities to be synced.
    ///
    /// NOTE: This will establish a new subscription stream with the server.
    pub async fn remove_entities_to_sync(&self, entities: Vec<EntityModel>) -> Result<(), Error> {
        self.subscribed_entities.remove_entities(entities)?;

        let updated_entities =
            self.subscribed_entities.entities.read().clone().into_iter().collect();
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
        entities: Vec<EntityModel>,
    ) -> Result<EntityUpdateStreaming, Error> {
        let mut grpc_client = self.inner.write().await;
        let stream = grpc_client.subscribe_entities(entities).await?;
        Ok(stream)
    }

    async fn initiate_entity(&self, model: &str, keys: Vec<FieldElement>) -> Result<(), Error> {
        let model_reader = self.world_reader.model(model).await?;
        let values = model_reader.entity_storage(&keys).await?;
        self.storage.set_entity_storage(
            cairo_short_string_to_felt(model).map_err(ParseError::CairoShortStringToFelt)?,
            keys,
            values,
        )?;
        Ok(())
    }
}

pub struct ClientBuilder {
    initial_entities_to_sync: Option<Vec<EntityModel>>,
}

impl ClientBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self { initial_entities_to_sync: None }
    }

    #[must_use]
    pub fn set_entities_to_sync(mut self, entities: Vec<EntityModel>) -> Self {
        self.initial_entities_to_sync = Some(entities);
        self
    }

    /// Returns an initialized [Client] with the provided configurations.
    ///
    /// The subscription service is not immediately started when calling this function, instead it
    /// must be manually started using `Client::start_subscription`.
    pub async fn build(
        self,
        torii_endpoint: String,
        // TODO: remove RPC reliant
        rpc_url: String,
        world: FieldElement,
    ) -> Result<Client, Error> {
        let mut grpc_client = torii_grpc::client::WorldClient::new(torii_endpoint, world).await?;

        let metadata = grpc_client.metadata().await?;

        let shared_metadata: Arc<_> = RwLock::new(metadata).into();
        let client_storage: Arc<_> = ModelStorage::new(shared_metadata.clone()).into();
        let subbed_entities: Arc<_> = SubscribedEntities::new(shared_metadata.clone()).into();

        // initialize the entities to be synced with the latest values
        let rpc_url = url::Url::parse(&rpc_url).map_err(ParseError::Url)?;
        let provider = JsonRpcClient::new(HttpTransport::new(rpc_url));
        let world_reader = WorldContractReader::new(world, provider);

        if let Some(entities_to_sync) = self.initial_entities_to_sync.clone() {
            subbed_entities.add_entities(entities_to_sync)?;

            // TODO: change this to querying the gRPC endpoint instead
            let subbed_entities = subbed_entities.entities.read().clone();
            for EntityModel { model, keys } in subbed_entities {
                let model_reader = world_reader.model(&model).await?;
                let values = model_reader.entity_storage(&keys).await?;

                client_storage.set_entity_storage(
                    cairo_short_string_to_felt(&model).unwrap(),
                    keys,
                    values,
                )?;
            }
        }

        Ok(Client {
            world_reader,
            storage: client_storage,
            metadata: shared_metadata,
            sub_client_handle: OnceCell::new(),
            inner: AsyncRwLock::new(grpc_client),
            subscribed_entities: subbed_entities,
        })
    }
}

impl Default for ClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}
