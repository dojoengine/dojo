pub mod error;
pub mod storage;
pub mod subscription;

use std::sync::Arc;

use dojo_types::component::EntityComponent;
use dojo_types::WorldMetadata;
use futures::channel::mpsc;
use parking_lot::{Mutex, RwLock};
use starknet_crypto::FieldElement;

use self::error::Error;
use self::storage::ComponentStorage;
use self::subscription::{SubscribedEntities, SubscriptionClientHandle};
use crate::client::subscription::SubscriptionClient;

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
    synced_entities: Arc<SubscribedEntities>,
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
    pub fn synced_entities(&self) -> Vec<EntityComponent> {
        self.synced_entities.entities.read().clone().into_iter().collect()
    }

    // pub async fn start_sync(&self) -> Result<SubscriptionClient, Error> {
    //     // initiate the stream any way, even if we don't have any initial entities to sync
    //     let sub_res_stream = self.inner.lock().subscribe_entities(self.synced_entities()).await?;
    //     // setup the subscription client
    //     let (sub_req_tx, sub_req_rcv) = mpsc::channel(128);
    //     let handle = SubscriptionClientHandle { event_handler: sub_req_tx };
    //     *self.subscription_client_handle.lock() = Some(handle);

    //     futures::sele
    // }
}

// TODO: able to handle entities that has not been set yet, currently `build` will panic if the
// `entities_to_sync` has never been set (the sql table isnt exist)
pub struct ClientBuilder {
    initial_entities_to_sync: Option<Vec<EntityComponent>>,
}

impl ClientBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self { initial_entities_to_sync: None }
    }

    pub async fn build(self, endpoint: String, world: FieldElement) -> Result<Client, Error> {
        let mut grpc_client = torii_grpc::client::WorldClient::new(endpoint, world).await?;

        let metadata = grpc_client.metadata().await?;

        let shared_metadata: Arc<_> = RwLock::new(metadata).into();
        let client_storage: Arc<_> = ComponentStorage::new(shared_metadata.clone()).into();
        let subbed_entities: Arc<_> = SubscribedEntities::new(shared_metadata.clone()).into();

        if let Some(entities_to_sync) = self.initial_entities_to_sync.clone() {
            subbed_entities.add_entities(entities_to_sync)?;

            // initialize the entities to be synced with the latest values
            let entities = subbed_entities.entities.read().clone();
            for EntityComponent { component, keys } in entities {
                let values = grpc_client.get_entity(component.clone(), keys.clone()).await?;
                println!("initial values: {:?}", values);
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

            let handle = tokio::task::spawn(SubscriptionClient {
                sub_res_stream,
                err_callback: None,
                req_rcv: sub_req_rcv,
                storage: client_storage.clone(),
                world_metadata: shared_metadata.clone(),
                subscribed_entities: subbed_entities.clone(),
            });
            SubscriptionClientHandle { event_handler: sub_req_tx, handle }
        };

        Ok(Client {
            storage: client_storage,
            metadata: shared_metadata,
            subscription_client_handle,
            inner: Mutex::new(grpc_client),
            synced_entities: subbed_entities,
        })
    }

    #[must_use]
    pub fn set_entities_to_sync(mut self, entities: Vec<EntityComponent>) -> Self {
        self.initial_entities_to_sync = Some(entities);
        self
    }
}

impl Default for ClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::thread;
    use std::time::Duration;

    use dojo_types::component::EntityComponent;
    use starknet::macros::felt;

    #[tokio::test(flavor = "multi_thread")]
    async fn main() {
        let world = felt!("0x103dd611b410c2aafc47f435e3141950b9d18e801e518209b7d28f6ff993f54");

        let client = super::ClientBuilder::new()
            .set_entities_to_sync(vec![EntityComponent {
                component: "Position".into(),
                keys: vec![felt!(
                    "0x517ececd29116499f4a1b64b094da79ba08dfd54a3edaa316134c41f8160973"
                )],
            }])
            .build("http://localhost:50051".to_string(), world)
            .await
            .unwrap();

        let values = client
            .entity(
                "Position".into(),
                vec![felt!("0x517ececd29116499f4a1b64b094da79ba08dfd54a3edaa316134c41f8160973")],
            )
            .unwrap();
        println!(
            "values: {}",
            values.iter().map(|v| format!("{v:#x}")).collect::<Vec<String>>().join(",")
        );

        loop {
            thread::sleep(Duration::from_secs(3));
            let values = client
                .entity(
                    "Position".into(),
                    vec![felt!(
                        "0x517ececd29116499f4a1b64b094da79ba08dfd54a3edaa316134c41f8160973"
                    )],
                )
                .unwrap();
            println!(
                "values: {}",
                values.iter().map(|v| format!("{v:#x}")).collect::<Vec<String>>().join(",")
            );
        }
    }
}
