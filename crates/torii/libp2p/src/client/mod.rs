use std::collections::HashMap;

use futures::channel::mpsc::UnboundedSender;
use futures::StreamExt;
use libp2p::gossipsub::{self, IdentTopic, MessageId};
use libp2p::swarm::{NetworkBehaviour, Swarm, SwarmEvent};
use libp2p::{identify, identity, noise, ping, tcp, yamux, Multiaddr, PeerId};
use tracing::info;

pub mod events;
use crate::client::events::ClientEvent;
use crate::errors::Error;
use crate::types::{ClientMessage, ServerMessage};

#[derive(NetworkBehaviour)]
#[behaviour(out_event = "ClientEvent")]
pub struct Behaviour {
    gossipsub: gossipsub::Behaviour,
    identify: identify::Behaviour,
    ping: ping::Behaviour,
}

pub struct Libp2pClient {
    pub swarm: Swarm<Behaviour>,
    pub topics: HashMap<String, IdentTopic>,
}

pub type Message = (PeerId, MessageId, ServerMessage);

impl Libp2pClient {
    pub fn new(relay_addr: String) -> Result<Self, Error> {
        let local_key = identity::Keypair::generate_ed25519();
        let peer_id = PeerId::from(local_key.public());

        info!(target: "libp2p", "Local peer id: {:?}", peer_id);

        let mut swarm = libp2p::SwarmBuilder::with_existing_identity(local_key)
            .with_tokio()
            .with_tcp(tcp::Config::default(), noise::Config::new, yamux::Config::default)?
            .with_behaviour(|key| {
                let gossipsub_config: gossipsub::Config = gossipsub::ConfigBuilder::default()
                    .heartbeat_interval(std::time::Duration::from_secs(10))
                    .build()
                    .expect("Gossipsup config is invalid");

                Behaviour {
                    gossipsub: gossipsub::Behaviour::new(
                        gossipsub::MessageAuthenticity::Signed(key.clone()),
                        gossipsub_config,
                    )
                    .expect("Gossipsub behaviour is invalid"),
                    identify: identify::Behaviour::new(identify::Config::new(
                        "/torii-client/0.0.1".to_string(),
                        key.public(),
                    )),
                    ping: ping::Behaviour::new(ping::Config::default()),
                }
            })?
            .build();

        info!(target: "libp2p", "Dialing relay: {:?}", relay_addr);
        swarm.dial(relay_addr.parse::<Multiaddr>()?)?;

        Ok(Self { swarm, topics: HashMap::new() })
    }

    pub async fn run(&mut self, sender: &UnboundedSender<Message>) {
        loop {
            // Poll the swarm for new events.
            let event = self.swarm.select_next_some().await;
            match event {
                SwarmEvent::Behaviour(event) => {
                    // Handle behaviour events.
                    if let ClientEvent::Gossipsub(gossipsub::Event::Message {
                        propagation_source: peer_id,
                        message_id,
                        message,
                    }) = event
                    {
                        // deserialize message
                        let message: ServerMessage = serde_json::from_slice(&message.data)
                            .expect("Failed to deserialize message");
                        sender.unbounded_send((peer_id, message_id, message)).unwrap();
                    }
                }
                SwarmEvent::ConnectionClosed { cause: Some(cause), .. } => {
                    tracing::info!("Swarm event: {:?}", cause);

                    if let libp2p::swarm::ConnectionError::KeepAliveTimeout = cause {
                        break;
                    }
                }
                evt => tracing::info!("Swarm event: {:?}", evt),
            }
        }
    }

    pub fn subscribe(&mut self, room: &str) -> Result<bool, Error> {
        let topic = IdentTopic::new(room);
        let sub = self.swarm.behaviour_mut().gossipsub.subscribe(&IdentTopic::new(room))?;
        if sub {
            self.topics.insert(room.to_string(), topic);
        }

        Ok(sub)
    }

    pub fn unsubscribe(&mut self, room: &str) -> Result<bool, Error> {
        let topic = IdentTopic::new(room);
        let unsub = self.swarm.behaviour_mut().gossipsub.unsubscribe(&topic)?;
        if unsub {
            self.topics.remove(room);
        }

        Ok(unsub)
    }

    pub fn publish(&mut self, message: &ClientMessage) -> Result<MessageId, Error> {
        self.swarm
            .behaviour_mut()
            .gossipsub
            .publish(IdentTopic::new("message"), serde_json::to_string(message).unwrap())
            .map_err(Error::PublishError)
    }
}
