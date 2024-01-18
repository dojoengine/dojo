use futures::StreamExt;
use libp2p::{
    gossipsub::{self, IdentTopic, MessageId, PublishError, SubscriptionError, TopicHash}, identify, identity, noise, ping, relay, swarm::{NetworkBehaviour, Swarm, SwarmEvent}, tcp, yamux, Multiaddr, PeerId
};
use std::error::Error;

mod events;
use crate::client::events::ClientEvent;

#[derive(NetworkBehaviour)]
#[behaviour(out_event = "ClientEvent")]
pub struct Behaviour {
    gossipsub: gossipsub::Behaviour,
    identify: identify::Behaviour,
    ping: ping::Behaviour,
}

pub struct RelayClient {
    swarm: Swarm<Behaviour>,
}

impl RelayClient {
    pub fn new(relay_addr: Multiaddr) -> Result<Self, Box<dyn Error>> {
        let local_key = identity::Keypair::generate_ed25519();
        let peer_id = PeerId::from(local_key.public());

        let mut swarm = libp2p::SwarmBuilder::with_existing_identity(local_key)
            .with_async_std()
            .with_tcp(tcp::Config::default(), noise::Config::new, || yamux::Config::default())?
            .with_behaviour(|key| {
                let gossipsub_config = gossipsub::ConfigBuilder::default()
                    .heartbeat_interval(std::time::Duration::from_secs(10))
                    .build()
                    .expect("Gossipsub configuration should be valid");

                Behaviour {
                    gossipsub: gossipsub::Behaviour::new(
                        gossipsub::MessageAuthenticity::Signed(key.clone()),
                        gossipsub_config,
                    )
                    .expect("Valid gossipsub behaviour"),
                    identify: identify::Behaviour::new(identify::Config::new(
                        "/torii-client/0.0.1".to_string(),
                        key.public(),
                    )),
                    ping: ping::Behaviour::new(ping::Config::default()),
                }
            })?
            .build();

        swarm.dial(relay_addr)?;

        Ok(Self { swarm })
    }

    pub async fn run(&mut self) {
        while let Some(event) = self.swarm.next().await {
            match event {
                SwarmEvent::Behaviour(ClientEvent::Identify(event)) => {
                    println!("Identify event: {:?}", event);
                }
                SwarmEvent::Behaviour(ClientEvent::Ping(event)) => {
                    println!("Ping event: {:?}", event);
                }
                SwarmEvent::Behaviour(ClientEvent::Gossipsub(message)) => {
                    // check if event type Message
                    match message {
                        gossipsub::Event::Message { propagation_source, message_id, message } => {
                            println!("Received: {:?}", String::from_utf8_lossy(&message.data));
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }

    pub fn topics(&self) -> Vec<&TopicHash> {
        self.swarm.behaviour().gossipsub.topics().collect()
    }



    pub fn subscribe(&mut self, room: &str) -> Result<bool, SubscriptionError> {
        self.swarm.behaviour_mut().gossipsub.subscribe(&IdentTopic::new(room))
    }

    pub fn unsubscribe(&mut self, room: &str) -> Result<bool, PublishError> {
        self.swarm.behaviour_mut().gossipsub.unsubscribe(&IdentTopic::new(room))
    }

    pub fn publish(&mut self, room: &str, message: &str) -> Result<MessageId, PublishError> {
        self.swarm.behaviour_mut().gossipsub.publish(IdentTopic::new(room), message.as_bytes())
    }
}
