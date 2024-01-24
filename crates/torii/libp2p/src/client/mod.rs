use std::time::Duration;

use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender};
use futures::{select, SinkExt, StreamExt};
use libp2p::gossipsub::{self, IdentTopic, MessageId};
use libp2p::swarm::{NetworkBehaviour, Swarm, SwarmEvent};
use libp2p::{identify, identity, ping, Multiaddr, PeerId};
#[cfg(not(target_arch = "wasm32"))]
use libp2p::{noise, tcp, yamux};
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

pub struct RelayClient {
    pub command_sender: UnboundedSender<Command>,
    pub message_receiver: UnboundedReceiver<Message>,
    pub event_loop: EventLoop,
}

pub struct EventLoop {
    swarm: Swarm<Behaviour>,
    message_sender: UnboundedSender<Message>,
    command_receiver: UnboundedReceiver<Command>,
}

pub type Message = (PeerId, MessageId, ServerMessage);
#[derive(Debug)]
pub enum Command {
    Subscribe(String),
    Unsubscribe(String),
    Publish(ClientMessage),
}

impl RelayClient {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new(relay_addr: String) -> Result<Self, Error> {
        let local_key = identity::Keypair::generate_ed25519();
        let peer_id = PeerId::from(local_key.public());

        info!(target: "torii::relay::client", peer_id = %peer_id, "Local peer id");

        let mut swarm = libp2p::SwarmBuilder::with_existing_identity(local_key)
            .with_tokio()
            .with_tcp(tcp::Config::default(), noise::Config::new, yamux::Config::default)?
            .with_quic()
            .with_behaviour(|key| {
                let gossipsub_config: gossipsub::Config = gossipsub::ConfigBuilder::default()
                    .heartbeat_interval(Duration::from_secs(10))
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
            .with_swarm_config(|cfg| cfg.with_idle_connection_timeout(Duration::from_secs(60)))
            .build();

        info!(target: "torii::relay::client", addr = %relay_addr, "Dialing relay");
        swarm.dial(relay_addr.parse::<Multiaddr>()?)?;

        let (message_sender, message_receiver) = futures::channel::mpsc::unbounded();
        let (command_sender, command_receiver) = futures::channel::mpsc::unbounded();
        Ok(Self {
            command_sender,
            message_receiver,
            event_loop: EventLoop { swarm, message_sender, command_receiver },
        })
    }

    #[cfg(target_arch = "wasm32")]
    pub fn new(relay_addr: String) -> Result<Self, Error> {
        let local_key = identity::Keypair::generate_ed25519();
        let peer_id = PeerId::from(local_key.public());

        info!(target: "torii::relay::client", peer_id = %peer_id, "Local peer id");

        let mut swarm = libp2p::SwarmBuilder::with_existing_identity(local_key)
            .with_wasm_bindgen()
            .with_other_transport(|key| {
                libp2p_webrtc_websys::Transport::new(libp2p_webrtc_websys::Config::new(&key))
            })
            .expect("Failed to create WebRTC transport")
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
            .with_swarm_config(|cfg| cfg.with_idle_connection_timeout(Duration::from_secs(60)))
            .build();

        info!(target: "torii::relay::client", addr = %relay_addr, "Dialing relay");
        swarm.dial(relay_addr.parse::<Multiaddr>()?)?;

        let (message_sender, message_receiver) = futures::channel::mpsc::unbounded();
        let (command_sender, command_receiver) = futures::channel::mpsc::unbounded();
        Ok(Self {
            command_sender,
            message_receiver,
            event_loop: EventLoop { swarm, message_sender, command_receiver },
        })
    }
}

impl EventLoop {
    pub async fn run(&mut self) {
        loop {
            // Poll the swarm for new events.
            select! {
                command = self.command_receiver.select_next_some() => {
                    match command {
                        Command::Subscribe(room) => {
                            self.subscribe(&room).expect("Failed to subscribe");
                        },
                        Command::Unsubscribe(room) => {
                            self.unsubscribe(&room).expect("Failed to unsubscribe");
                        },
                        Command::Publish(message) => {
                            self.publish(&message).expect("Failed to publish message");
                        },
                    }
                },
                event = self.swarm.select_next_some() => {
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
                                self.message_sender.send((peer_id, message_id, message)).await.expect("Failed to send message");
                            }
                        }
                        SwarmEvent::ConnectionClosed { cause: Some(cause), .. } => {
                            info!(target: "torii::relay::client", cause = ?cause, "Connection closed");

                            if let libp2p::swarm::ConnectionError::KeepAliveTimeout = cause {
                                info!(target: "torii::relay::client", "Connection closed due to keep alive timeout. Shutting down client.");
                                return;
                            }
                        }
                        evt => {
                            info!(target: "torii::relay::client", event = ?evt, "Unhandled event");
                        }
                    }
                },
            }
        }
    }

    fn subscribe(&mut self, room: &str) -> Result<bool, Error> {
        let topic = IdentTopic::new(room);
        let sub = self.swarm.behaviour_mut().gossipsub.subscribe(&topic)?;
        // if sub {
        //     self.topics.insert(room.to_string(), topic);
        // }

        Ok(sub)
    }

    fn unsubscribe(&mut self, room: &str) -> Result<bool, Error> {
        let topic = IdentTopic::new(room);
        let unsub = self.swarm.behaviour_mut().gossipsub.unsubscribe(&topic)?;
        // if unsub {
        //     self.topics.remove(room);
        // }

        Ok(unsub)
    }

    fn publish(&mut self, message: &ClientMessage) -> Result<MessageId, Error> {
        self.swarm
            .behaviour_mut()
            .gossipsub
            .publish(IdentTopic::new("message"), serde_json::to_string(message).unwrap())
            .map_err(Error::PublishError)
    }
}
