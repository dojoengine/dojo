use std::{
    collections::HashMap,
    pin::Pin,
    task::{Context, Poll},
};

use futures::{
    channel::mpsc::{UnboundedReceiver, UnboundedSender},
    Future, Stream, StreamExt,
};
use libp2p::{
    gossipsub::{self, IdentTopic, MessageId, PublishError, SubscriptionError},
    identify, identity, noise, ping,
    swarm::{NetworkBehaviour, Swarm, SwarmEvent},
    tcp, yamux, Multiaddr, PeerId,
};

pub mod events;
use crate::{errors::Error, types::ServerMessage};
use crate::{client::events::ClientEvent, types::ClientMessage};

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

impl Libp2pClient {
    pub fn new(relay_addr: String) -> Result<Self, Error> {
        let relay_addr = relay_addr.parse::<Multiaddr>()?;

        let local_key = identity::Keypair::generate_ed25519();
        let peer_id = PeerId::from(local_key.public());

        let mut swarm = libp2p::SwarmBuilder::with_existing_identity(local_key)
            .with_async_std()
            .with_tcp(tcp::Config::default(), noise::Config::new, || yamux::Config::default())?
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

        swarm.dial(relay_addr)?;

        Ok(Self { swarm, topics: HashMap::new() })
    }

    pub async fn run_message_listener(&mut self, sender: &UnboundedSender<ServerMessage>) {
        loop {
            // Poll the swarm for new events.
            match self.swarm.next().await {
                Some(event) => {
                    match event {
                        SwarmEvent::Behaviour(event) => {
                            // Handle behaviour events.
                            match event {
                                ClientEvent::Gossipsub(gossipsub::Event::Message {
                                    propagation_source: peer_id,
                                    message_id,
                                    message,
                                }) => {
                                    // deserialize message
                                    let message: ServerMessage =
                                        serde_json::from_slice(&message.data)
                                            .expect("Failed to deserialize message");
                                    sender.unbounded_send(message).unwrap();
                                }
                                _ => {}
                            }
                        }
                        // You can handle other types of SwarmEvents here
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }

    pub fn subscribe(&mut self, room: &str) -> Result<bool, Error> {
        let topic = IdentTopic::new(room);
        let sub = self.swarm.behaviour_mut().gossipsub.subscribe(&IdentTopic::new(room));
        if let Ok(_) = sub {
            self.topics.insert(room.to_string(), topic);
        }

        sub.map_err(Error::SubscriptionError)
    }

    pub fn unsubscribe(&mut self, room: &str) -> Result<bool, Error> {
        let topic = IdentTopic::new(room);
        let unsub = self.swarm.behaviour_mut().gossipsub.unsubscribe(&topic);
        if let Ok(_) = unsub {
            self.topics.remove(room);
        }

        unsub.map_err(Error::PublishError)
    }

    pub fn publish(&mut self, message: &ClientMessage) -> Result<MessageId, Error> {
        self.swarm
            .behaviour_mut()
            .gossipsub
            .publish(IdentTopic::new("message"), serde_json::to_string(message).unwrap())
            .map_err(Error::PublishError)
    }
}
