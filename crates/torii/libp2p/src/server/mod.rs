use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::net::Ipv4Addr;
use std::path::Path;
use std::time::Duration;
use std::{fs, io};

use futures::StreamExt;
use libp2p::core::multiaddr::Protocol;
use libp2p::core::muxing::StreamMuxerBox;
use libp2p::core::Multiaddr;
use libp2p::gossipsub::{self, IdentTopic};
use libp2p::swarm::{NetworkBehaviour, SwarmEvent};
use libp2p::{identify, identity, noise, ping, relay, tcp, yamux, PeerId, Swarm, Transport};
use libp2p_webrtc as webrtc;
use rand::thread_rng;
use tracing::info;
use webrtc::tokio::Certificate;

use crate::constants;
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

pub struct Relay {
    swarm: Swarm<Behaviour>,
}

impl Relay {
    pub fn new(
        port: u16,
        port_webrtc: u16,
        local_key_path: Option<String>,
        cert_path: Option<String>,
    ) -> Result<Self, Error> {
        let local_key = if let Some(path) = local_key_path {
            let path = Path::new(&path);
            read_or_create_identity(path).map_err(Error::ReadIdentityError)?
        } else {
            identity::Keypair::generate_ed25519()
        };

        let cert = if let Some(path) = cert_path {
            let path = Path::new(&path);
            read_or_create_certificate(path).map_err(Error::ReadCertificateError)?
        } else {
            Certificate::generate(&mut thread_rng()).unwrap()
        };

        info!(target: "torii::relay::server", peer_id = %PeerId::from(local_key.public()), "Relay peer id");

        let mut swarm = libp2p::SwarmBuilder::with_existing_identity(local_key)
            .with_tokio()
            .with_tcp(tcp::Config::default(), noise::Config::new, yamux::Config::default)?
            .with_quic()
            .with_other_transport(|key| {
                Ok(webrtc::tokio::Transport::new(key.clone(), cert)
                    .map(|(peer_id, conn), _| (peer_id, StreamMuxerBox::new(conn))))
            })
            .expect("Failed to create WebRTC transport")
            .with_behaviour(|key| {
                let message_id_fn = |message: &gossipsub::Message| {
                    let mut s = DefaultHasher::new();
                    message.data.hash(&mut s);
                    gossipsub::MessageId::from(s.finish().to_string())
                };
                let gossipsub_config = gossipsub::ConfigBuilder::default()
                        .heartbeat_interval(Duration::from_secs(constants::GOSSIPSUB_HEARTBEAT_INTERVAL_SECS)) // This is set to aid debugging by not cluttering the log space
                        .validation_mode(gossipsub::ValidationMode::Strict) // This sets the kind of message validation. The default is Strict (enforce message signing)
                        .message_id_fn(message_id_fn) // content-address messages. No two messages of the same content will be propagated.
                        .build()
                        .map_err(|msg| io::Error::new(io::ErrorKind::Other, msg)).unwrap(); // Temporary hack because `build` does not return a proper `std::error::Error`.

                Behaviour {
                    relay: relay::Behaviour::new(key.public().to_peer_id(), Default::default()),
                    ping: ping::Behaviour::new(ping::Config::new()),
                    identify: identify::Behaviour::new(identify::Config::new(
                        "/torii-relay/0.0.1".to_string(),
                        key.public(),
                    )),
                    gossipsub: gossipsub::Behaviour::new(
                        gossipsub::MessageAuthenticity::Signed(key.clone()),
                        gossipsub_config,
                    )
                    .unwrap(),
                }
            })?
            .with_swarm_config(|cfg| {
                cfg.with_idle_connection_timeout(Duration::from_secs(
                    constants::IDLE_CONNECTION_TIMEOUT_SECS,
                ))
            })
            .build();

        // TCP
        let listen_addr_tcp = Multiaddr::from(Ipv4Addr::UNSPECIFIED).with(Protocol::Tcp(port));
        swarm.listen_on(listen_addr_tcp.clone())?;

        // UDP QUIC
        let listen_addr_quic =
            Multiaddr::from(Ipv4Addr::UNSPECIFIED).with(Protocol::Udp(port)).with(Protocol::QuicV1);
        swarm.listen_on(listen_addr_quic.clone())?;

        // WebRTC
        let listen_addr_webrtc = Multiaddr::from(Ipv4Addr::UNSPECIFIED)
            .with(Protocol::Udp(port_webrtc))
            .with(Protocol::WebRTCDirect);
        swarm.listen_on(listen_addr_webrtc.clone())?;

        // Clients will send their messages to the "message" topic
        // with a room name as the message data.
        // and we will forward those messages to a specific room - in this case the topic
        // along with the message data.
        swarm
            .behaviour_mut()
            .gossipsub
            .subscribe(&IdentTopic::new(constants::MESSAGING_TOPIC))
            .unwrap();

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
                            // Deserialize message.
                            // We shouldn't panic here
                            let message = serde_json::from_slice::<ClientMessage>(&message.data);
                            if let Err(e) = message {
                                info!(
                                    target: "torii::relay::server::gossipsub",
                                    error = %e,
                                    "Failed to deserialize message"
                                );
                                continue;
                            }

                            let message = message.unwrap();

                            info!(
                                target: "torii::relay::server",
                                message_id = %message_id,
                                peer_id = %peer_id,
                                topic = %message.topic,
                                data = %String::from_utf8_lossy(&message.data),
                                "Received message"
                            );

                            // forward message to room
                            let server_message =
                                ServerMessage { peer_id: peer_id.to_bytes(), data: message.data };

                            if let Err(e) = self.swarm.behaviour_mut().gossipsub.publish(
                                IdentTopic::new(message.topic),
                                serde_json::to_string(&server_message)
                                    .expect("Failed to serialize message")
                                    .as_bytes(),
                            ) {
                                info!(
                                    target: "torii::relay::server::gossipsub",
                                    error = %e,
                                    "Failed to publish message"
                                );
                            }
                        }
                        ServerEvent::Gossipsub(gossipsub::Event::Subscribed { peer_id, topic }) => {
                            info!(
                                target: "torii::relay::server::gossipsub",
                                peer_id = %peer_id,
                                topic = %topic,
                                "Subscribed to topic"
                            );
                        }
                        ServerEvent::Gossipsub(gossipsub::Event::Unsubscribed {
                            peer_id,
                            topic,
                        }) => {
                            info!(
                                target: "torii::relay::server::gossipsub",
                                peer_id = %peer_id,
                                topic = %topic,
                                "Unsubscribed from topic"
                            );
                        }
                        ServerEvent::Identify(identify::Event::Received {
                            info: identify::Info { observed_addr, .. },
                            peer_id,
                        }) => {
                            info!(
                                target: "torii::relay::server::identify",
                                peer_id = %peer_id,
                                observed_addr = %observed_addr,
                                "Received identify event"
                            );
                            self.swarm.add_external_address(observed_addr.clone());
                        }
                        ServerEvent::Ping(ping::Event { peer, result, .. }) => {
                            info!(
                                target: "torii::relay::server::ping",
                                peer_id = %peer,
                                result = ?result,
                                "Received ping event"
                            );
                        }
                        _ => {}
                    }
                }
                SwarmEvent::NewListenAddr { address, .. } => {
                    info!(target: "torii::relay::server", address = %address, "New listen address");
                }
                _ => {}
            }
        }
    }
}

fn read_or_create_identity(path: &Path) -> anyhow::Result<identity::Keypair> {
    if path.exists() {
        let bytes = fs::read(path)?;

        info!(target: "torii::relay::server", path = %path.display(), "Using existing identity");

        return Ok(identity::Keypair::from_protobuf_encoding(&bytes)?); // This only works for ed25519 but that is what we are using.
    }

    let identity = identity::Keypair::generate_ed25519();

    fs::write(path, identity.to_protobuf_encoding()?)?;

    info!(target: "torii::relay::server", path = %path.display(), "Generated new identity");

    Ok(identity)
}

fn read_or_create_certificate(path: &Path) -> anyhow::Result<Certificate> {
    if path.exists() {
        let pem = fs::read_to_string(path)?;

        info!(target: "torii::relay::server", path = %path.display(), "Using existing certificate");

        return Ok(Certificate::from_pem(&pem)?);
    }

    let cert = Certificate::generate(&mut rand::thread_rng())?;
    fs::write(path, cert.serialize_pem().as_bytes())?;

    info!(target: "torii::relay::server", path = %path.display(), "Generated new certificate");

    Ok(cert)
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    #[tokio::test]
    async fn test_read_or_create_identity() {
        let dir = tempdir().unwrap();
        let identity_path = dir.path().join("identity");

        // Test identity creation
        let identity1 = read_or_create_identity(&identity_path).unwrap();
        assert!(identity_path.exists());

        // Test identity reading
        let identity2 = read_or_create_identity(&identity_path).unwrap();
        assert_eq!(identity1.public(), identity2.public());

        dir.close().unwrap();
    }

    #[tokio::test]
    async fn test_read_or_create_certificate() {
        let dir = tempdir().unwrap();
        let cert_path = dir.path().join("certificate");

        // Test certificate creation
        let cert1 = read_or_create_certificate(&cert_path).unwrap();
        assert!(cert_path.exists());

        // Test certificate reading
        let cert2 = read_or_create_certificate(&cert_path).unwrap();
        assert_eq!(cert1.serialize_pem(), cert2.serialize_pem());

        dir.close().unwrap();
    }
}
