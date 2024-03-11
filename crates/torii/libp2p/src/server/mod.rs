use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::net::Ipv4Addr;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use std::{fs, io};

use crypto_bigint::U256;
use dojo_types::primitive::Primitive;
use dojo_types::schema::{Member, Struct, Ty};
use futures::StreamExt;
use indexmap::IndexMap;
use libp2p::core::multiaddr::Protocol;
use libp2p::core::muxing::StreamMuxerBox;
use libp2p::core::Multiaddr;
use libp2p::gossipsub::{self, IdentTopic};
use libp2p::swarm::{NetworkBehaviour, SwarmEvent};
use libp2p::{identify, identity, noise, ping, relay, tcp, yamux, PeerId, Swarm, Transport};
use libp2p_webrtc as webrtc;
use rand::thread_rng;
use starknet_crypto::{poseidon_hash_many, verify, Signature};
use starknet_ff::FieldElement;
use tokio::sync::RwLock;
use torii_core::sql::Sql;
use tracing::{info, warn};
use webrtc::tokio::Certificate;

use crate::constants;
use crate::errors::Error;

mod events;

use crate::server::events::ServerEvent;
use crate::typed_data::{get_value_type, Field, PrimitiveType, TypedData};
use crate::types::Message;
use sqlx::Row;

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
    pool: Arc<RwLock<Sql>>,
}

impl Relay {
    pub fn new(
        pool: Arc<RwLock<Sql>>,
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

        Ok(Self { swarm, pool })
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
                            // Deserialize typed data.
                            // We shouldn't panic here
                            let data = match serde_json::from_slice::<Message>(&message.data) {
                                Ok(message) => message,
                                Err(e) => {
                                    info!(
                                        target: "torii::relay::server::gossipsub",
                                        error = %e,
                                        "Failed to deserialize message"
                                    );
                                    continue;
                                }
                            };

                            let parsed_message = match validate_message(&data.message.message) {
                                Ok(parsed_message) => parsed_message,
                                Err(e) => {
                                    info!(
                                        target: "torii::relay::server::gossipsub",
                                        error = %e,
                                        "Failed to validate message"
                                    );
                                    continue;
                                }
                            };

                            info!(
                                target: "torii::relay::server",
                                message_id = %message_id,
                                peer_id = %peer_id,
                                data = ?data,
                                "Received message"
                            );

                            // retrieve entity identity from db
                            let mut pool = match self.pool.read().await.pool.acquire().await {
                                Ok(pool) => pool,
                                Err(e) => {
                                    warn!(
                                        target: "torii::relay::server",
                                        error = %e,
                                        "Failed to acquire pool"
                                    );
                                    continue;
                                }
                            };

                            // select only identity field, if doesn't exist, empty string
                            let entity = match sqlx::query(
                                "SELECT * FROM ? WHERE id = ?",
                            )
                            .bind(parsed_message.model_name)
                            .bind(parsed_message.hashed_keys.to_string())
                            .fetch_optional(&mut *pool)
                            .await
                            {
                                Ok(entity_identity) => entity_identity,
                                Err(e) => {
                                    warn!(
                                        target: "torii::relay::server",
                                        error = %e,
                                        "Failed to fetch entity"
                                    );
                                    continue;
                                }
                            };

                            if entity.is_none() {
                                // we can set the entity without checking identity


                            }

                            let entity = entity.unwrap();
                            let identity = match entity.try_get::<String, _>("identity") {
                                Ok(identity) => identity,
                                Err(e) => {
                                    warn!(
                                        target: "torii::relay::server",
                                        error = %e,
                                        "Failed to get identity from model"
                                    );
                                    continue;
                                }
                            };



                            
                            let entity_identity = match FieldElement::from_str(&identity) {
                                Ok(entity_identity) => entity_identity,
                                Err(e) => {
                                    warn!(
                                        target: "torii::relay::server",
                                        error = %e,
                                        "Failed to parse entity identity"
                                    );
                                    continue;
                                }
                            };

                            // Verify the signature
                            let message_hash =
                                if let Ok(message) = data.message.encode(entity_identity) {
                                    message
                                } else {
                                    info!(
                                        target: "torii::relay::server",
                                        "Failed to encode message"
                                    );
                                    continue;
                                };

                            // for the public key used for verification; use identity from model
                            if let Ok(valid) = verify(
                                &entity_identity,
                                &message_hash,
                                &data.signature_r,
                                &data.signature_s,
                            ) {
                                if !valid {
                                    info!(
                                        target: "torii::relay::server",
                                        "Invalid signature"
                                    );
                                    continue;
                                }
                            } else {
                                info!(
                                    target: "torii::relay::server",
                                    "Failed to verify signature"
                                );
                                continue;
                            }

                            // if let Err(e) = self
                            //     .pool
                            //     .write()
                            //     .await
                            //     // event id is message id
                            //     .set_entity(, &message_id.to_string())
                            //     .await
                            // {
                            //     info!(
                            //         target: "torii::relay::server",
                            //         error = %e,
                            //         "Failed to set message"
                            //     );
                            // }
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

struct ParsedMessage {
    hashed_keys: FieldElement,
    model: Ty,
}

struct TyValue {
    r#type: String,
    value: PrimitiveType,
    key: bool
}

fn parse_object_to_ty(name: String, object: &IndexMap<String, PrimitiveType>) -> Result<Ty, Error> {
    let mut ty_struct = Struct {
        name,
        children: vec![]
    };
    
    for (field_name, value) in object {
        // value has to be of type object
        let object = if let PrimitiveType::Object(object) = value {
            object
        } else {
            return Err(Error::InvalidMessageError("Value is not an object".to_string()));
        };

        let r#type = if let Some(r#type) = object.get("type") {
            if let PrimitiveType::String(r#type) = r#type {
                r#type
            } else {
                return Err(Error::InvalidMessageError("Type is not a string".to_string()));
            }
        } else {
            return Err(Error::InvalidMessageError("Type is missing".to_string()));
        };

        let value = if let Some(value) = object.get("value") {
            value
        } else {
            return Err(Error::InvalidMessageError("Value is missing".to_string()));
        };

        let key = if let Some(key) = object.get("key") {
            if let PrimitiveType::Bool(key) = key {
                *key
            } else {
                return Err(Error::InvalidMessageError("Key is not a boolean".to_string()));
            }
        } else {
            return Err(Error::InvalidMessageError("Key is missing".to_string()));
        };

        match value {
            PrimitiveType::Object(object) => {
                let ty = parse_object_to_ty(field_name.clone(), object)?;
                ty_struct.children.push(Member {
                    name: field_name.clone(),
                    ty,
                    key
                });
            }
            PrimitiveType::Array(array) => {
                // tuples not supported yet
                unimplemented!()
            }
            PrimitiveType::Number(number) => {
                ty_struct.children.push(Member {
                    name: field_name.clone(),
                    ty: match r#type.as_str() {
                        "u8" => Ty::Primitive(Primitive::U8(Some(number.as_u64().unwrap() as u8))),
                        "u16" => Ty::Primitive(Primitive::U16(Some(number.as_u64().unwrap() as u16))),
                        "u32" => Ty::Primitive(Primitive::U32(Some(number.as_u64().unwrap() as u32))),
                        "usize" => Ty::Primitive(Primitive::USize(Some(number.as_u64().unwrap() as u32))),
                        "u64" => Ty::Primitive(Primitive::U64(Some(number.as_u64().unwrap()))),
                    },
                    key
                });
            }
            PrimitiveType::Bool(boolean) => {
                ty_struct.children.push(Member {
                    name: field_name.clone(),
                    ty: Ty::Primitive(Primitive::Bool(Some(*boolean))),
                    key
                });
            }
            PrimitiveType::String(string) => {
                match r#type.as_str() {
                    "u128" => {
                        ty_struct.children.push(Member {
                            name: field_name.clone(),
                            ty: Ty::Primitive(Primitive::U128(Some(u128::from_str(string).unwrap()))),
                            key
                        });
                    }
                    "u256" => {
                        ty_struct.children.push(Member {
                            name: field_name.clone(),
                            ty: Ty::Primitive(Primitive::U256(Some(U256::from_be_hex(string)))),
                            key
                        });
                    }
                    "felt" => {
                        ty_struct.children.push(Member {
                            name: field_name.clone(),
                            ty: Ty::Primitive(Primitive::Felt252(Some(FieldElement::from_str(string).unwrap()))),
                            key
                        });
                    }
                    "class_hash" => {
                        ty_struct.children.push(Member {
                            name: field_name.clone(),
                            ty: Ty::Primitive(Primitive::ClassHash(Some(FieldElement::from_str(string).unwrap()))),
                            key
                        });
                    }
                    "contract_address" => {
                        ty_struct.children.push(Member {
                            name: field_name.clone(),
                            ty: Ty::Primitive(Primitive::ContractAddress(Some(FieldElement::from_str(string).unwrap()))),
                            key
                        });
                    }
                }
            }
        }
    }

    Ok(Ty::Struct(ty_struct))
}

// Validates the message model
// and returns the identity and signature
fn validate_message(message: &IndexMap<String, PrimitiveType>) -> Result<ParsedMessage, Error> {
    let model_name = if let Some(model_name) = message.get("model") {
        if let PrimitiveType::String(model_name) = model_name {
            model_name
        } else {
            return Err(Error::InvalidMessageError("Model name is not a string".to_string()));
        }
    } else {
        return Err(Error::InvalidMessageError("Model name is missing".to_string()));
    };

    let hashed_keys = if let Some(hashed_keys) = message.get("hashed_keys") {
        if let PrimitiveType::String(hashed_keys) = hashed_keys {
            if let Ok(hashed_keys) = FieldElement::from_str(hashed_keys) {
                hashed_keys
            } else {
                return Err(Error::InvalidMessageError(
                    "Hashed keys is not a valid field element".to_string(),
                ));
            }
        } else {
            return Err(Error::InvalidMessageError("Hashed keys is not a string".to_string()));
        }
    } else {
        return Err(Error::InvalidMessageError("Hashed keys is missing".to_string()));
    };

    let model = if let Some(object) = message.get(model_name) {
        if let PrimitiveType::Object(object) = object {
            parse_object_to_ty(model_name.clone(), object)?
        } else {
            return Err(Error::InvalidMessageError("Model is not a struct".to_string()));
        }
    } else {
        return Err(Error::InvalidMessageError("Model is missing".to_string()));
    };

    Ok(ParsedMessage { model, hashed_keys })
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
