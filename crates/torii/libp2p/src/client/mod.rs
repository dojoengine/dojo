use std::sync::Arc;
use std::time::Duration;

use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender};
use futures::channel::oneshot;
use futures::lock::Mutex;
use futures::{select, StreamExt};
use libp2p::gossipsub::{self, IdentTopic, MessageId};
use libp2p::swarm::{NetworkBehaviour, Swarm, SwarmEvent};
use libp2p::{identify, identity, noise, ping, yamux, Multiaddr, PeerId};
use tracing::info;

pub mod events;
use crate::client::events::ClientEvent;
use crate::constants;
use crate::error::Error;
use crate::types::Message;

pub(crate) const LOG_TARGET: &str = "torii::relay::client";

#[derive(NetworkBehaviour)]
#[behaviour(out_event = "ClientEvent")]
struct Behaviour {
    gossipsub: gossipsub::Behaviour,
    identify: identify::Behaviour,
    ping: ping::Behaviour,
}

#[derive(Debug)]
pub struct RelayClient {
    pub command_sender: CommandSender,
    pub event_loop: Arc<Mutex<EventLoop>>,
}

#[allow(missing_debug_implementations)]
pub struct EventLoop {
    swarm: Swarm<Behaviour>,
    command_receiver: UnboundedReceiver<Command>,
}

#[derive(Debug)]
enum Command {
    Publish(Message, oneshot::Sender<Result<MessageId, Error>>),
}

impl RelayClient {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new(relay_addr: String) -> Result<Self, Error> {
        use libp2p::core::muxing::StreamMuxerBox;
        use libp2p::core::upgrade::Version;
        use libp2p::{dns, tcp, websocket, Transport};
        use libp2p_webrtc as webrtc;
        use libp2p_webrtc::tokio::Certificate;
        use rand::thread_rng;

        let local_key = identity::Keypair::generate_ed25519();
        let peer_id = PeerId::from(local_key.public());

        info!(target: LOG_TARGET, peer_id = %peer_id, "Local peer id.");

        let cert = Certificate::generate(&mut thread_rng()).unwrap();
        let mut swarm = libp2p::SwarmBuilder::with_existing_identity(local_key)
            .with_tokio()
            .with_tcp(tcp::Config::default(), noise::Config::new, yamux::Config::default)?
            .with_quic()
            .with_other_transport(|key| {
                webrtc::tokio::Transport::new(key.clone(), cert)
                    .map(|(peer_id, conn), _| (peer_id, StreamMuxerBox::new(conn)))
            })
            .expect("Failed to create WebRTC transport")
            .with_other_transport(|key| {
                let transport = websocket::Config::new(
                    dns::tokio::Transport::system(tcp::tokio::Transport::new(
                        tcp::Config::default(),
                    ))
                    .unwrap(),
                );

                transport
                    .upgrade(Version::V1)
                    .authenticate(noise::Config::new(key).unwrap())
                    .multiplex(yamux::Config::default())
            })
            .expect("Failed to create WebSocket transport")
            .with_dns()
            .expect("Failed to create DNS transport")
            .with_behaviour(build_behaviour)?
            .with_swarm_config(build_swarm_config)
            .build();

        info!(target: LOG_TARGET, addr = %relay_addr, "Dialing relay.");
        swarm.dial(relay_addr.parse::<Multiaddr>()?)?;

        let (command_sender, command_receiver) = futures::channel::mpsc::unbounded();
        Ok(Self {
            command_sender: CommandSender::new(command_sender),
            event_loop: Arc::new(Mutex::new(EventLoop { swarm, command_receiver })),
        })
    }

    #[cfg(target_arch = "wasm32")]
    pub fn new(relay_addr: String) -> Result<Self, Error> {
        use libp2p::core::upgrade::Version;
        use libp2p::core::Transport;

        let local_key = identity::Keypair::generate_ed25519();
        let peer_id = PeerId::from(local_key.public());

        info!(target: LOG_TARGET, peer_id = %peer_id, "Local peer id.");

        // WebRTC transport and WebTransport are not natively supported in NodeJS, so we need to check if we are in a
        // browser environment
        let mut swarm = match web_sys::window() {
            Some(_) => libp2p::SwarmBuilder::with_existing_identity(local_key.clone())
                .with_wasm_bindgen()
                .with_other_transport(|key| {
                    libp2p_webtransport_websys::Transport::new(libp2p_webtransport_websys::Config::new(&key))
                })
                .expect("Failed to create WebTransport transport")
                .with_other_transport(|key| {
                    libp2p_webrtc_websys::Transport::new(libp2p_webrtc_websys::Config::new(&key))
                })
                .expect("Failed to create WebRTC transport")
                .with_other_transport(|key| {
                    libp2p_websocket_websys::Transport::default()
                        .upgrade(Version::V1)
                        .authenticate(noise::Config::new(&key).unwrap())
                        .multiplex(yamux::Config::default())
                        .boxed()
                })
                .expect("Failed to create WebSocket transport")
                .with_behaviour(build_behaviour)?
                .with_swarm_config(build_swarm_config)
                .build(),
            None => libp2p::SwarmBuilder::with_existing_identity(local_key)
                .with_wasm_bindgen()
                // NodeJS natively implements WebSocket transport, so we should be able to use it.
                .with_other_transport(|key| {
                    libp2p_websocket_websys::Transport::default()
                        .upgrade(Version::V1)
                        .authenticate(noise::Config::new(&key).unwrap())
                        .multiplex(yamux::Config::default())
                        .boxed()
                })
                .expect("Failed to create WebSocket transport")
                .with_behaviour(build_behaviour)?
                .with_swarm_config(build_swarm_config)
                .build(),
        };

        info!(target: LOG_TARGET, addr = %relay_addr, "Dialing relay.");
        swarm.dial(relay_addr.parse::<Multiaddr>()?)?;

        let (command_sender, command_receiver) = futures::channel::mpsc::unbounded();
        Ok(Self {
            command_sender: CommandSender::new(command_sender),
            event_loop: Arc::new(Mutex::new(EventLoop { swarm, command_receiver })),
        })
    }
}

#[derive(Debug)]
pub struct CommandSender {
    sender: UnboundedSender<Command>,
}

impl CommandSender {
    fn new(sender: UnboundedSender<Command>) -> Self {
        Self { sender }
    }

    pub async fn publish(&self, data: Message) -> Result<MessageId, Error> {
        let (tx, rx) = oneshot::channel();

        self.sender.unbounded_send(Command::Publish(data, tx)).expect("Failed to send command");

        rx.await.expect("Failed to receive response")
    }
}

impl EventLoop {
    async fn handle_command(
        &mut self,
        command: Command,
        is_relay_ready: bool,
        commands_queue: Arc<Mutex<Vec<Command>>>,
    ) {
        match command {
            Command::Publish(data, sender) => {
                // if the relay is not ready yet, add the message to the queue
                if !is_relay_ready {
                    commands_queue.lock().await.push(Command::Publish(data, sender));
                } else {
                    sender.send(self.publish(&data)).expect("Failed to send response");
                }
            }
        }
    }

    pub async fn run(&mut self) {
        let mut is_relay_ready = false;
        let commands_queue = Arc::new(Mutex::new(Vec::new()));

        loop {
            // Poll the swarm for new events.
            select! {
                command = self.command_receiver.select_next_some() => {
                    self.handle_command(command, is_relay_ready, commands_queue.clone()).await;
                },
                event = self.swarm.select_next_some() => {
                    match event {
                        SwarmEvent::Behaviour(ClientEvent::Gossipsub(gossipsub::Event::Subscribed { topic, .. })) => {
                            // Handle behaviour events.
                            info!(target: LOG_TARGET, topic = ?topic, "Relay ready. Received subscription confirmation.");

                            if !is_relay_ready {
                                is_relay_ready = true;

                                // Execute all the commands that were queued while the relay was not ready.
                                for command in commands_queue.lock().await.drain(..) {
                                    self.handle_command(command, is_relay_ready, commands_queue.clone()).await;
                                }
                            }
                        }
                        SwarmEvent::ConnectionClosed { cause: Some(cause), .. } => {
                            info!(target: LOG_TARGET, cause = ?cause, "Connection closed.");

                            if let libp2p::swarm::ConnectionError::KeepAliveTimeout = cause {
                                info!(target: LOG_TARGET, "Connection closed due to keep alive timeout. Shutting down client.");
                                return;
                            }
                        }
                        _ => {}
                    }
                },
            }
        }
    }

    fn publish(&mut self, data: &Message) -> Result<MessageId, Error> {
        self.swarm
            .behaviour_mut()
            .gossipsub
            .publish(
                IdentTopic::new(constants::MESSAGING_TOPIC),
                serde_json::to_string(data).unwrap(),
            )
            .map_err(Error::PublishError)
    }
}

fn build_behaviour(key: &libp2p::identity::Keypair) -> Behaviour {
    let gossipsub_config: gossipsub::Config = gossipsub::ConfigBuilder::default()
        .heartbeat_interval(Duration::from_secs(constants::GOSSIPSUB_HEARTBEAT_INTERVAL_SECS))
        .build()
        .expect("Gossipsup config is invalid");

    Behaviour {
        gossipsub: gossipsub::Behaviour::new(
            gossipsub::MessageAuthenticity::Signed(key.clone()),
            gossipsub_config,
        )
        .expect("Gossipsub behaviour is invalid"),
        identify: identify::Behaviour::new(identify::Config::new(
            format!("/torii-client/{}", env!("CARGO_PKG_VERSION")),
            key.public(),
        )),
        ping: ping::Behaviour::new(ping::Config::default()),
    }
}

fn build_swarm_config(config: libp2p::swarm::Config) -> libp2p::swarm::Config {
    config
        .with_idle_connection_timeout(Duration::from_secs(constants::IDLE_CONNECTION_TIMEOUT_SECS))
}
