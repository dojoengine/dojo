pub mod error;
pub mod storage;
pub mod subscription;

use std::cell::OnceCell;
use std::sync::Arc;

use dojo_types::packing::unpack;
use dojo_types::schema::{EntityModel, Ty};
use dojo_types::WorldMetadata;
use parking_lot::RwLock;
use starknet::core::types::{BlockId, BlockTag};
use starknet::core::utils::cairo_short_string_to_felt;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::JsonRpcClient;
use starknet_crypto::FieldElement;

use self::error::{Error, ParseError};
use self::storage::ModelStorage;
use self::subscription::{SubscribedEntities, SubscriptionClientHandle};
use crate::client::subscription::SubscriptionService;
use crate::contract::world::WorldContractReader;

// TODO: expose the World interface from the `Client`
#[allow(unused)]
pub struct Client {
    /// Metadata of the World that the client is connected to.
    metadata: Arc<RwLock<WorldMetadata>>,
    /// The grpc client.
    inner: torii_grpc::client::WorldClient,
    /// Entity storage
    storage: Arc<ModelStorage>,
    /// Entities the client are subscribed to.
    subscribed_entities: Arc<SubscribedEntities>,
    /// The subscription client handle.
    sub_client_handle: OnceCell<SubscriptionClientHandle>,
}

impl Client {
    /// Returns a [ClientBuilder] for building a [Client].
    pub fn build() -> ClientBuilder {
        ClientBuilder::new()
    }

    /// Returns the metadata of the world that the client is connected to.
    pub fn metadata(&self) -> WorldMetadata {
        self.metadata.read().clone()
    }

    /// Returns the model value of an entity.
    pub fn entity(&self, model: &str, keys: &[FieldElement]) -> Option<Ty> {
        let Ok(Some(raw_values)) =
            self.storage.get_entity(cairo_short_string_to_felt(model).ok()?, keys)
        else {
            return None;
        };

        let mut schema = self.metadata.read().model(model).map(|m| m.schema.clone())?;
        let layout = self.metadata.read().model(model).map(|m| m.layout.clone())?;

        let unpacked = unpack(raw_values, layout).unwrap();
        let mut keys_and_unpacked = [keys.to_vec(), unpacked].concat();

        schema.deserialize(&mut keys_and_unpacked).unwrap();

        Some(schema)
    }

    /// Returns the list of entities that the client is subscribed to.
    pub fn synced_entities(&self) -> Vec<EntityModel> {
        self.subscribed_entities.entities.read().clone().into_iter().collect()
    }

    /// Initiate the entity subscriptions and returns a [SubscriptionService] which when await'ed
    /// will execute the subscription service and starts the syncing process.
    pub async fn start_subscription(&mut self) -> Result<SubscriptionService, Error> {
        let sub_res_stream = self.inner.subscribe_entities(self.synced_entities()).await?;

        let (service, handle) = SubscriptionService::new(
            Arc::clone(&self.storage),
            Arc::clone(&self.metadata),
            Arc::clone(&self.subscribed_entities),
            sub_res_stream,
        );

        self.sub_client_handle.set(handle).unwrap();
        Ok(service)
    }
}

// TODO: able to handle entities that has not been set yet, currently `build` will panic if the
// `entities_to_sync` has never been set (the sql table isnt exist)
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

        if let Some(entities_to_sync) = self.initial_entities_to_sync.clone() {
            subbed_entities.add_entities(entities_to_sync)?;

            // initialize the entities to be synced with the latest values
            let rpc_url = url::Url::parse(&rpc_url).map_err(ParseError::Url)?;
            let provider = JsonRpcClient::new(HttpTransport::new(rpc_url));
            let world_reader = WorldContractReader::new(world, &provider);

            // TODO: change this to querying the gRPC endpoint instead
            let subbed_entities = subbed_entities.entities.read().clone();
            for EntityModel { model, keys } in subbed_entities {
                let model_reader =
                    world_reader.model(&model, BlockId::Tag(BlockTag::Pending)).await?;
                let values = model_reader
                    .entity_storage(keys.clone(), BlockId::Tag(BlockTag::Pending))
                    .await?;

                client_storage.set_entity(
                    cairo_short_string_to_felt(&model).unwrap(),
                    keys,
                    values,
                )?;
            }
        }

        Ok(Client {
            inner: grpc_client,
            storage: client_storage,
            metadata: shared_metadata,
            sub_client_handle: OnceCell::new(),
            subscribed_entities: subbed_entities,
        })
    }
}

impl Default for ClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}
