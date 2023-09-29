pub mod error;
pub mod storage;
pub mod subscription;

use std::sync::Arc;

use dojo_types::schema::EntityModel;
use dojo_types::WorldMetadata;
use futures::channel::mpsc;
use parking_lot::{Mutex, RwLock};
use starknet::core::types::{BlockId, BlockTag};
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::JsonRpcClient;
use starknet_crypto::FieldElement;
#[cfg(not(target_arch = "wasm32"))]
use tokio::task::spawn as spawn_task;
use url::Url;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_futures::spawn_local as spawn_task;

use self::error::Error;
use self::storage::ComponentStorage;
use self::subscription::{SubscribedEntities, SubscriptionClientHandle};
use crate::client::subscription::SubscriptionClient;
use crate::contract::world::WorldContractReader;

// TODO: expose the World interface from the `Client`
#[allow(unused)]
pub struct Client {
    /// Metadata of the World that the client is connected to.
    metadata: Arc<RwLock<WorldMetadata>>,
    /// The grpc client.
    inner: Mutex<torii_grpc::client::WorldClient>,
    /// Entity storage
    storage: Arc<ComponentStorage>,
    /// Entities the client are subscribed to.
    entity_subscription: Arc<SubscribedEntities>,
    /// The subscription client handle
    subscription_client_handle: SubscriptionClientHandle,
}

impl Client {
    /// Returns the metadata of the world.
    pub fn world_metadata(&self) -> WorldMetadata {
        self.metadata.read().clone()
    }

    /// Returns the component value of an entity.
    pub fn entity(&self, component: String, keys: Vec<FieldElement>) -> Option<Vec<FieldElement>> {
        self.storage.get_entity((component, keys))
    }

    /// Returns the list of entities that the client is subscribed to.
    pub fn synced_entities(&self) -> Vec<EntityModel> {
        self.entity_subscription.entities.read().clone().into_iter().collect()
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

    pub async fn build(
        self,
        torii_endpoint: String,
        // TODO: remove RPC
        rpc_url: String,
        world: FieldElement,
    ) -> Result<Client, Error> {
        let mut grpc_client = torii_grpc::client::WorldClient::new(torii_endpoint, world).await?;

        let metadata = grpc_client.metadata().await?;

        let shared_metadata: Arc<_> = RwLock::new(metadata).into();
        let client_storage: Arc<_> = ComponentStorage::new(shared_metadata.clone()).into();
        let subbed_entities: Arc<_> = SubscribedEntities::new(shared_metadata.clone()).into();

        if let Some(entities_to_sync) = self.initial_entities_to_sync.clone() {
            subbed_entities.add_entities(entities_to_sync)?;

            // initialize the entities to be synced with the latest values
            let provider = JsonRpcClient::new(HttpTransport::new(Url::parse(&rpc_url)?));
            let world_reader = WorldContractReader::new(world, &provider);

            // TODO: change this to querying the gRPC endpoint instead
            let subbed_entities = subbed_entities.entities.read().clone();
            for EntityModel { model: component, keys } in subbed_entities {
                let component_reader =
                    world_reader.model(&component, BlockId::Tag(BlockTag::Pending)).await?;
                let values =
                    component_reader.entity(keys.clone(), BlockId::Tag(BlockTag::Pending)).await?;
                client_storage.set_entity((component, keys), values)?;
            }
        }

        // initiate the stream any way, even if we don't have any initial entities to sync
        let sub_res_stream = grpc_client
            .subscribe_entities(self.initial_entities_to_sync.unwrap_or_default())
            .await?;
        // setup the subscription client
        let subscription_client_handle = {
            let (sub_req_tx, sub_req_rcv) = mpsc::channel(128);

            spawn_task(SubscriptionClient {
                sub_res_stream,
                err_callback: None,
                req_rcv: sub_req_rcv,
                storage: client_storage.clone(),
                world_metadata: shared_metadata.clone(),
                subscribed_entities: subbed_entities.clone(),
            });

            SubscriptionClientHandle { event_handler: sub_req_tx }
        };

        Ok(Client {
            storage: client_storage,
            metadata: shared_metadata,
            subscription_client_handle,
            inner: Mutex::new(grpc_client),
            entity_subscription: subbed_entities,
        })
    }

    #[must_use]
    pub fn set_entities_to_sync(mut self, entities: Vec<EntityModel>) -> Self {
        self.initial_entities_to_sync = Some(entities);
        self
    }
}

impl Default for ClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}
