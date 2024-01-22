use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io;
use std::net::Ipv4Addr;
use std::time::Duration;

use futures::future::{select, Either};
use futures::StreamExt;
use libp2p::core::multiaddr::Protocol;
use libp2p::core::Multiaddr;
use libp2p::gossipsub::{self, IdentTopic};
use libp2p::swarm::{NetworkBehaviour, SwarmEvent};
use libp2p::{
    core::muxing::StreamMuxerBox, identify, identity, noise, ping, relay,
    tcp, yamux, StreamProtocol, Swarm, Transport,
};
use libp2p_webrtc as webrtc;
use libp2p_webrtc::tokio::Certificate;
use rand::thread_rng;
use tracing::info;

use crate::errors::Error;

mod events;

use crate::server::events::ServerEvent;
use crate::types::{ClientMessage, ServerMessage};

#[derive(NetworkBehaviour)]
#[behaviour(out_event = "ServerEvent")]
pub struct Behaviour {
    relay: relay::Behaviour,
    ping: ping::Behaviour,
    identify: identify::Behaviour,
    gossipsub: gossipsub::Behaviour,
}

pub struct Libp2pRelay {
    swarm: Swarm<Behaviour>,
}

impl Libp2pRelay {
    pub fn new(port: u16, port_webrtc: u16) -> Result<Self, Error> {
        let local_key: identity::Keypair = identity::Keypair::generate_ed25519();

        let mut swarm = libp2p::SwarmBuilder::with_existing_identity(local_key)
            .with_tokio()
            .with_tcp(tcp::Config::default(), noise::Config::new, yamux::Config::default)?
            .with_quic()
            .with_other_transport(|key| {
                Ok(webrtc::tokio::Transport::new(
                    key.clone(),
                    webrtc::tokio::Certificate::generate(&mut thread_rng())?,
                )
                .map(|(peer_id, conn), _| (peer_id, StreamMuxerBox::new(conn))))
            }).unwrap()
            .with_behaviour(|key| {
                let message_id_fn = |message: &gossipsub::Message| {
                    let mut s = DefaultHasher::new();
                    message.data.hash(&mut s);
                    gossipsub::MessageId::from(s.finish().to_string())
                };
                let gossipsub_config = gossipsub::ConfigBuilder::default()
                        .heartbeat_interval(Duration::from_secs(10)) // This is set to aid debugging by not cluttering the log space
                        .validation_mode(gossipsub::ValidationMode::Strict) // This sets the kind of message validation. The default is Strict (enforce message signing)
                        .message_id_fn(message_id_fn) // content-address messages. No two messages of the same content will be propagated.
                        .build()
                        .map_err(|msg| io::Error::new(io::ErrorKind::Other, msg)).unwrap(); // Temporary hack because `build` does not return a proper `std::error::Error`.

                Behaviour {
                    relay: relay::Behaviour::new(key.public().to_peer_id(), Default::default()),
                    ping: ping::Behaviour::new(ping::Config::new()),
                    identify: identify::Behaviour::new(identify::Config::new(
                        "/TODO/0.0.1".to_string(),
                        key.public(),
                    )),
                    gossipsub: gossipsub::Behaviour::new(
                        gossipsub::MessageAuthenticity::Signed(key.clone()),
                        gossipsub_config,
                    )
                    .unwrap(),
                }
            })?
            .build();

        // TCP
        let listen_addr_tcp = Multiaddr::from(Ipv4Addr::UNSPECIFIED).with(Protocol::Tcp(port));
        swarm.listen_on(listen_addr_tcp)?;

        // UDP QUIC
        let listen_addr_quic =
            Multiaddr::from(Ipv4Addr::UNSPECIFIED).with(Protocol::Udp(port)).with(Protocol::QuicV1);
        swarm.listen_on(listen_addr_quic)?;

        // WebRTC
        let listen_addr_webrtc = Multiaddr::from(Ipv4Addr::UNSPECIFIED)
            .with(Protocol::Udp(port_webrtc))
            .with(Protocol::WebRTCDirect);
        swarm.listen_on(listen_addr_webrtc)?;

        // Clients will send their messages to the "message" topic
        // with a room name as the message data.
        // and we will forward those messages to a specific room - in this case the topic
        // along with the message data.
        swarm.behaviour_mut().gossipsub.subscribe(&IdentTopic::new("message")).unwrap();

        Ok(Self { swarm })
    }

    pub async fn run(&mut self) {
        loop {
            match self.swarm.next().await.expect("Infinite Stream.") {
                SwarmEvent::Behaviour(event) => {
                    match &event {
                        ServerEvent::Gossipsub(gossipsub::Event::Message {
                            propagation_source: peer_id,
                            message_id,
                            message,
                        }) => {
                            // deserialize message
                            let message: ClientMessage = serde_json::from_slice(&message.data)
                                .expect("Failed to deserialize message");

                            info!(target: "libp2p", "Received message {:?} from peer {:?} with topic {:?} and data {:?}", message_id, peer_id, message.topic, message.data);

                            // forward message to room
                            let server_message =
                                ServerMessage { peer_id: peer_id.to_string(), data: message.data };
                            self.swarm
                                .behaviour_mut()
                                .gossipsub
                                .publish(
                                    IdentTopic::new(message.topic),
                                    serde_json::to_string(&server_message)
                                        .expect("Failed to serialize message")
                                        .as_bytes(),
                                )
                                .expect("Failed to publish message");
                        }
                        ServerEvent::Identify(identify::Event::Received {
                            info: identify::Info { observed_addr, .. },
                            peer_id,
                        }) => {
                            info!(target: "libp2p", "Received identify event from peer {:?} with observed address {:?}", peer_id, observed_addr);
                            self.swarm.add_external_address(observed_addr.clone());
                        }
                        ServerEvent::Ping(ping::Event { peer, result, .. }) => {
                            info!(target: "libp2p", "Ping success from peer {:?} with result {:?}", peer, result);
                        }
                        _ => {}
                    }
                }
                SwarmEvent::NewListenAddr { address, .. } => {
                    info!(target: "libp2p", "Listening on {:?}", address);
                }
                _ => {}
            }
        }
    }
}
