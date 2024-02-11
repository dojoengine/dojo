use std::sync::Arc;
use std::time::Duration;

use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender};
use futures::channel::oneshot;
use futures::lock::Mutex;
use futures::{select, StreamExt};
use libp2p::gossipsub::{self, IdentTopic, MessageId, TopicHash};
use libp2p::swarm::{NetworkBehaviour, Swarm, SwarmEvent};
use libp2p::{identify, identity, ping, Multiaddr, PeerId};
#[cfg(not(target_arch = "wasm32"))]
use libp2p::{noise, tcp, yamux};
use tracing::info;

pub mod events;
use crate::client::events::ClientEvent;
use crate::constants;
use crate::errors::Error;
use crate::types::{ClientMessage, ServerMessage};

#[derive(NetworkBehaviour)]
#[behaviour(out_event = "ClientEvent")]
struct Behaviour {
    gossipsub: gossipsub::Behaviour,
    identify: identify::Behaviour,
    ping: ping::Behaviour,
}

pub struct RelayClient {
    pub message_receiver: Arc<Mutex<UnboundedReceiver<Message>>>,
    pub command_sender: CommandSender,
    pub event_loop: Arc<Mutex<EventLoop>>,
}

pub struct EventLoop {
    swarm: Swarm<Behaviour>,
    message_sender: UnboundedSender<Message>,
    command_receiver: UnboundedReceiver<Command>,
}

#[derive(Debug, Clone)]
pub struct Message {
    // PeerId of the relay that propagated the message
    pub propagation_source: PeerId,
    // Peer that published the message
    pub source: PeerId,
    pub message_id: MessageId,
    // Hash of the topic message was published to
    pub topic: TopicHash,
    // Raw message payload
    pub data: Vec<u8>,
}

#[derive(Debug)]
enum Command {
    Subscribe(String, oneshot::Sender<Result<bool, Error>>),
    Unsubscribe(String, oneshot::Sender<Result<bool, Error>>),
    Publish(String, Vec<u8>, oneshot::Sender<Result<MessageId, Error>>),
    WaitForRelay(oneshot::Sender<Result<(), Error>>),
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
                    .heartbeat_interval(Duration::from_secs(
                        constants::GOSSIPSUB_HEARTBEAT_INTERVAL_SECS,
                    ))
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
            .with_swarm_config(|cfg| {
                cfg.with_idle_connection_timeout(Duration::from_secs(
                    constants::IDLE_CONNECTION_TIMEOUT_SECS,
                ))
            })
            .build();

        info!(target: "torii::relay::client", addr = %relay_addr, "Dialing relay");
        swarm.dial(relay_addr.parse::<Multiaddr>()?)?;

        let (message_sender, message_receiver) = futures::channel::mpsc::unbounded();
        let (command_sender, command_receiver) = futures::channel::mpsc::unbounded();
        Ok(Self {
            command_sender: CommandSender::new(command_sender),
            message_receiver: Arc::new(Mutex::new(message_receiver)),
            event_loop: Arc::new(Mutex::new(EventLoop { swarm, message_sender, command_receiver })),
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
            command_sender: CommandSender::new(command_sender),
            message_receiver: Arc::new(Mutex::new(message_receiver)),
            event_loop: Arc::new(Mutex::new(EventLoop { swarm, message_sender, command_receiver })),
        })
    }
}

pub struct CommandSender {
    sender: UnboundedSender<Command>,
}

impl CommandSender {
    fn new(sender: UnboundedSender<Command>) -> Self {
        Self { sender }
    }

    pub async fn subscribe(&mut self, room: String) -> Result<bool, Error> {
        let (tx, rx) = oneshot::channel();

        self.sender.unbounded_send(Command::Subscribe(room, tx)).expect("Failed to send command");

        rx.await.expect("Failed to receive response")
    }

    pub async fn unsubscribe(&mut self, room: String) -> Result<bool, Error> {
        let (tx, rx) = oneshot::channel();

        self.sender.unbounded_send(Command::Unsubscribe(room, tx)).expect("Failed to send command");

        rx.await.expect("Failed to receive response")
    }

    pub async fn publish(&mut self, topic: String, data: Vec<u8>) -> Result<MessageId, Error> {
        let (tx, rx) = oneshot::channel();

        self.sender
            .unbounded_send(Command::Publish(topic, data, tx))
            .expect("Failed to send command");

        rx.await.expect("Failed to receive response")
    }

    pub async fn wait_for_relay(&mut self) -> Result<(), Error> {
        let (tx, rx) = oneshot::channel();

        self.sender.unbounded_send(Command::WaitForRelay(tx)).expect("Failed to send command");

        rx.await.expect("Failed to receive response")
    }
}

impl EventLoop {
    pub async fn run(&mut self) {
        let mut is_relay_ready = false;
        let mut relay_ready_tx = None;

        loop {
            // Poll the swarm for new events.
            select! {
                command = self.command_receiver.select_next_some() => {
                    match command {
                        Command::Subscribe(room, sender) => {
                            sender.send(self.subscribe(&room)).expect("Failed to send response");
                        },
                        Command::Unsubscribe(room, sender) => {
                            sender.send(self.unsubscribe(&room)).expect("Failed to send response");
                        },
                        Command::Publish(topic, data, sender) => {
                            sender.send(self.publish(topic, data)).expect("Failed to send response");
                        },
                        Command::WaitForRelay(sender) => {
                            if is_relay_ready {
                                sender.send(Ok(())).expect("Failed to send response");
                            } else {
                                relay_ready_tx = Some(sender);
                            }
                        }
                    }
                },
                event = self.swarm.select_next_some() => {
                    match event {
                        SwarmEvent::Behaviour(event) => {
                            match event {
                                // Handle behaviour events.
                                ClientEvent::Gossipsub(gossipsub::Event::Message {
                                    propagation_source: peer_id,
                                    message_id,
                                    message,
                                }) => {
                                    // deserialize message payload
                                    let message_payload: ServerMessage = serde_json::from_slice(&message.data)
                                        .expect("Failed to deserialize message");

                                    let message = Message {
                                        propagation_source: peer_id,
                                        source: PeerId::from_bytes(&message_payload.peer_id).expect("Failed to parse peer id"),
                                        message_id,
                                        topic: message.topic,
                                        data: message_payload.data,
                                    };

                                    self.message_sender.unbounded_send(message).expect("Failed to send message");
                                }
                                ClientEvent::Gossipsub(gossipsub::Event::Subscribed { topic, .. }) => {
                                    info!(target: "torii::relay::client::gossipsub", topic = ?topic, "Relay ready. Received subscription confirmation");

                                    is_relay_ready = true;
                                    if let Some(tx) = relay_ready_tx.take() {
                                        tx.send(Ok(())).expect("Failed to send response");
                                    }
                                }
                                _ => {}
                            }
                        }
                        SwarmEvent::ConnectionClosed { cause: Some(cause), .. } => {
                            info!(target: "torii::relay::client", cause = ?cause, "Connection closed");

                            if let libp2p::swarm::ConnectionError::KeepAliveTimeout = cause {
                                info!(target: "torii::relay::client", "Connection closed due to keep alive timeout. Shutting down client.");
                                return;
                            }
                        }
                        _ => {}
                    }
                },
            }
        }
    }

    fn subscribe(&mut self, room: &str) -> Result<bool, Error> {
        let topic = IdentTopic::new(room);
        self.swarm.behaviour_mut().gossipsub.subscribe(&topic).map_err(Error::SubscriptionError)
    }

    fn unsubscribe(&mut self, room: &str) -> Result<bool, Error> {
        let topic = IdentTopic::new(room);
        self.swarm.behaviour_mut().gossipsub.unsubscribe(&topic).map_err(Error::PublishError)
    }

    fn publish(&mut self, topic: String, data: Vec<u8>) -> Result<MessageId, Error> {
        self.swarm
            .behaviour_mut()
            .gossipsub
            .publish(
                IdentTopic::new(constants::MESSAGING_TOPIC),
                serde_json::to_string(&ClientMessage { topic, data }).unwrap(),
            )
            .map_err(Error::PublishError)
    }
}
