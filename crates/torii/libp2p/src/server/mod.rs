use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::net::Ipv4Addr;
use std::path::Path;
use std::str::FromStr;
use std::time::Duration;
use std::{fs, io};

use chrono::Utc;
use crypto_bigint::U256;
use dojo_types::primitive::Primitive;
use dojo_types::schema::Ty;
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
use starknet::core::types::{BlockId, BlockTag, Felt, FunctionCall};
use starknet::core::utils::get_selector_from_name;
use starknet::providers::Provider;
use starknet_crypto::{poseidon_hash_many, verify};
use torii_core::sql::Sql;
use tracing::{info, warn};
use webrtc::tokio::Certificate;

use crate::constants;
use crate::errors::Error;

mod events;

use dojo_world::contracts::model::ModelReader;

use crate::server::events::ServerEvent;
use crate::typed_data::PrimitiveType;
use crate::types::Message;

pub(crate) const LOG_TARGET: &str = "torii::relay::server";

#[derive(NetworkBehaviour)]
#[behaviour(out_event = "ServerEvent")]
pub struct Behaviour {
    relay: relay::Behaviour,
    ping: ping::Behaviour,
    identify: identify::Behaviour,
    gossipsub: gossipsub::Behaviour,
}

pub struct Relay<P: Provider + Sync> {
    swarm: Swarm<Behaviour>,
    db: Sql,
    provider: Box<P>,
}

impl<P: Provider + Sync> Relay<P> {
    pub fn new(
        pool: Sql,
        provider: P,
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

        info!(target: LOG_TARGET, peer_id = %PeerId::from(local_key.public()), "Relay peer id.");

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
                // Hash messages by their content. No two messages of the same content will be
                // propagated.
                let _message_id_fn = |message: &gossipsub::Message| {
                    let mut s = DefaultHasher::new();
                    message.data.hash(&mut s);
                    gossipsub::MessageId::from(s.finish().to_string())
                };
                let gossipsub_config = gossipsub::ConfigBuilder::default()
                        .heartbeat_interval(Duration::from_secs(constants::GOSSIPSUB_HEARTBEAT_INTERVAL_SECS)) // This is set to aid debugging by not cluttering the log space
                        .validation_mode(gossipsub::ValidationMode::Strict) // This sets the kind of message validation. The default is Strict (enforce message signing)
                        // TODO: Use this once we incorporate nonces in the message model?
                        // .message_id_fn(message_id_fn) // content-address messages. No two messages of the same content will be propagated.
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

        Ok(Self { swarm, db: pool, provider: Box::new(provider) })
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
                                        target: LOG_TARGET,
                                        error = %e,
                                        "Deserializing message."
                                    );
                                    continue;
                                }
                            };

                            let ty = match validate_message(&self.db, &data.message.message).await {
                                Ok(parsed_message) => parsed_message,
                                Err(e) => {
                                    info!(
                                        target: LOG_TARGET,
                                        error = %e,
                                        "Validating message."
                                    );
                                    continue;
                                }
                            };

                            info!(
                                target: LOG_TARGET,
                                message_id = %message_id,
                                peer_id = %peer_id,
                                data = ?data,
                                "Received message."
                            );

                            // retrieve entity identity from db
                            let mut pool = match self.db.pool.acquire().await {
                                Ok(pool) => pool,
                                Err(e) => {
                                    warn!(
                                        target: LOG_TARGET,
                                        error = %e,
                                        "Acquiring pool."
                                    );
                                    continue;
                                }
                            };

                            let keys = match ty_keys(&ty) {
                                Ok(keys) => keys,
                                Err(e) => {
                                    warn!(
                                        target: LOG_TARGET,
                                        error = %e,
                                        "Retrieving message model keys."
                                    );
                                    continue;
                                }
                            };

                            // select only identity field, if doesn't exist, empty string
                            let query =
                                format!("SELECT external_identity FROM {} WHERE id = ?", ty.name());
                            let entity_identity: Option<String> = match sqlx::query_scalar(&query)
                                .bind(format!("{:#x}", poseidon_hash_many(&keys)))
                                .fetch_optional(&mut *pool)
                                .await
                            {
                                Ok(entity_identity) => entity_identity,
                                Err(e) => {
                                    warn!(
                                        target: LOG_TARGET,
                                        error = %e,
                                        "Fetching entity."
                                    );
                                    continue;
                                }
                            };

                            if entity_identity.is_none() {
                                // we can set the entity without checking identity
                                if let Err(e) = self
                                    .db
                                    .set_entity(
                                        ty,
                                        &message_id.to_string(),
                                        Utc::now().timestamp() as u64,
                                    )
                                    .await
                                {
                                    info!(
                                        target: LOG_TARGET,
                                        error = %e,
                                        "Setting message."
                                    );
                                    continue;
                                } else {
                                    info!(
                                        target: LOG_TARGET,
                                        message_id = %message_id,
                                        peer_id = %peer_id,
                                        "Message set."
                                    );
                                    continue;
                                }
                            }

                            let entity_identity = match Felt::from_str(&entity_identity.unwrap()) {
                                Ok(identity) => identity,
                                Err(e) => {
                                    warn!(
                                        target: LOG_TARGET,
                                        error = %e,
                                        "Parsing identity."
                                    );
                                    continue;
                                }
                            };

                            // TODO: have a nonce in model to check
                            // against entity nonce and message nonce
                            // to prevent replay attacks.

                            // Verify the signature
                            let message_hash =
                                if let Ok(message) = data.message.encode(entity_identity) {
                                    message
                                } else {
                                    info!(
                                        target: LOG_TARGET,
                                        "Encoding message."
                                    );
                                    continue;
                                };

                            let public_key = match self
                                .provider
                                .call(
                                    FunctionCall {
                                        contract_address: entity_identity,
                                        entry_point_selector: get_selector_from_name(
                                            "getPublicKey",
                                        )
                                        .unwrap(),
                                        calldata: vec![],
                                    },
                                    BlockId::Tag(BlockTag::Pending),
                                )
                                .await
                            {
                                Ok(res) => res[0],
                                Err(e) => {
                                    warn!(
                                        target: LOG_TARGET,
                                        error = %e,
                                        "Fetching public key."
                                    );
                                    continue;
                                }
                            };

                            if !match verify(
                                &public_key,
                                &message_hash,
                                &data.signature_r,
                                &data.signature_s,
                            ) {
                                Ok(valid) => valid,
                                Err(e) => {
                                    warn!(
                                        target: LOG_TARGET,
                                        error = %e,
                                        "Verifying signature."
                                    );
                                    continue;
                                }
                            } {
                                info!(
                                    target: LOG_TARGET,
                                    message_id = %message_id,
                                    peer_id = %peer_id,
                                    "Invalid signature."
                                );
                                continue;
                            }

                            if let Err(e) = self
                                .db
                                // event id is message id
                                .set_entity(
                                    ty,
                                    &message_id.to_string(),
                                    Utc::now().timestamp() as u64,
                                )
                                .await
                            {
                                info!(
                                    target: LOG_TARGET,
                                    error = %e,
                                    "Setting message."
                                );
                            }

                            info!(
                                target: LOG_TARGET,
                                message_id = %message_id,
                                peer_id = %peer_id,
                                "Message verified and set."
                            );
                        }
                        ServerEvent::Gossipsub(gossipsub::Event::Subscribed { peer_id, topic }) => {
                            info!(
                                target: LOG_TARGET,
                                peer_id = %peer_id,
                                topic = %topic,
                                "Subscribed to topic."
                            );
                        }
                        ServerEvent::Gossipsub(gossipsub::Event::Unsubscribed {
                            peer_id,
                            topic,
                        }) => {
                            info!(
                                target: LOG_TARGET,
                                peer_id = %peer_id,
                                topic = %topic,
                                "Unsubscribed from topic."
                            );
                        }
                        ServerEvent::Identify(identify::Event::Received {
                            info: identify::Info { observed_addr, .. },
                            peer_id,
                        }) => {
                            info!(
                                target: LOG_TARGET,
                                peer_id = %peer_id,
                                observed_addr = %observed_addr,
                                "Received identify event."
                            );
                            self.swarm.add_external_address(observed_addr.clone());
                        }
                        ServerEvent::Ping(ping::Event { peer, result, .. }) => {
                            info!(
                                target: LOG_TARGET,
                                peer_id = %peer,
                                result = ?result,
                                "Received ping event."
                            );
                        }
                        _ => {}
                    }
                }
                SwarmEvent::NewListenAddr { address, .. } => {
                    info!(target: LOG_TARGET, address = %address, "New listen address.");
                }
                event => {
                    info!(target: LOG_TARGET, event = ?event, "Unhandled event.");
                }
            }
        }
    }
}

fn ty_keys(ty: &Ty) -> Result<Vec<Felt>, Error> {
    if let Ty::Struct(s) = &ty {
        let mut keys = Vec::new();
        for m in s.keys() {
            keys.extend(m.serialize().map_err(|_| {
                Error::InvalidMessageError("Failed to serialize model key".to_string())
            })?);
        }
        Ok(keys)
    } else {
        Err(Error::InvalidMessageError("Entity is not a struct".to_string()))
    }
}

pub fn parse_value_to_ty(value: &PrimitiveType, ty: &mut Ty) -> Result<(), Error> {
    match value {
        PrimitiveType::Object(object) => match ty {
            Ty::Struct(struct_) => {
                for (key, value) in object {
                    let member =
                        struct_.children.iter_mut().find(|member| member.name == *key).ok_or_else(
                            || Error::InvalidMessageError(format!("Member {} not found", key)),
                        )?;

                    parse_value_to_ty(value, &mut member.ty)?;
                }
            }
            // U256 is an object with two u128 fields
            // low and high
            Ty::Primitive(Primitive::U256(u256)) => {
                let mut low = Ty::Primitive(Primitive::U128(None));
                let mut high = Ty::Primitive(Primitive::U128(None));

                // parse the low and high fields
                parse_value_to_ty(&object["low"], &mut low)?;
                parse_value_to_ty(&object["high"], &mut high)?;

                let low = low.as_primitive().unwrap().as_u128().unwrap();
                let high = high.as_primitive().unwrap().as_u128().unwrap();

                let mut bytes = [0u8; 32];
                bytes[..16].copy_from_slice(&high.to_be_bytes());
                bytes[16..].copy_from_slice(&low.to_be_bytes());

                *u256 = Some(U256::from_be_slice(&bytes));
            }
            // an enum is a SNIP-12 compliant object with a single key
            // where the K is the variant name
            // and the value is the variant value
            Ty::Enum(enum_) => {
                let (option_name, value) = object.first().ok_or_else(|| {
                    Error::InvalidMessageError("Enum variant not found".to_string())
                })?;

                enum_.options.iter_mut().for_each(|option| {
                    if option.name == *option_name {
                        parse_value_to_ty(value, &mut option.ty).unwrap();
                    }
                });

                enum_.set_option(option_name).map_err(|e| {
                    Error::InvalidMessageError(format!("Failed to set enum option: {}", e))
                })?;
            }
            _ => {
                return Err(Error::InvalidMessageError(format!(
                    "Invalid object type for {}",
                    ty.name()
                )));
            }
        },
        PrimitiveType::Array(values) => match ty {
            Ty::Array(array) => {
                let inner_type = array[0].clone();

                // clear the array, which contains the inner type
                array.clear();

                // parse each value to the inner type
                for value in values {
                    let mut ty = inner_type.clone();
                    parse_value_to_ty(value, &mut ty)?;
                    array.push(ty);
                }
            }
            Ty::Tuple(tuple) => {
                // our array values need to match the length of the tuple
                if tuple.len() != values.len() {
                    return Err(Error::InvalidMessageError("Tuple length mismatch".to_string()));
                }

                for (i, value) in tuple.iter_mut().enumerate() {
                    parse_value_to_ty(&values[i], value)?;
                }
            }
            _ => {
                return Err(Error::InvalidMessageError(format!(
                    "Invalid array type for {}",
                    ty.name()
                )));
            }
        },
        PrimitiveType::Number(number) => match ty {
            Ty::Primitive(primitive) => match *primitive {
                Primitive::U8(ref mut u8) => {
                    *u8 = Some(number.as_u64().unwrap() as u8);
                }
                Primitive::U16(ref mut u16) => {
                    *u16 = Some(number.as_u64().unwrap() as u16);
                }
                Primitive::U32(ref mut u32) => {
                    *u32 = Some(number.as_u64().unwrap() as u32);
                }
                Primitive::USize(ref mut usize) => {
                    *usize = Some(number.as_u64().unwrap() as u32);
                }
                Primitive::U64(ref mut u64) => {
                    *u64 = Some(number.as_u64().unwrap());
                }
                _ => {
                    return Err(Error::InvalidMessageError(format!(
                        "Invalid number type for {}",
                        ty.name()
                    )));
                }
            },
            _ => {
                return Err(Error::InvalidMessageError(format!(
                    "Invalid number type for {}",
                    ty.name()
                )));
            }
        },
        PrimitiveType::Bool(boolean) => {
            *ty = Ty::Primitive(Primitive::Bool(Some(*boolean)));
        }
        PrimitiveType::String(string) => match ty {
            Ty::Primitive(primitive) => match primitive {
                Primitive::U8(v) => {
                    *v = Some(u8::from_str(string).unwrap());
                }
                Primitive::U16(v) => {
                    *v = Some(u16::from_str(string).unwrap());
                }
                Primitive::U32(v) => {
                    *v = Some(u32::from_str(string).unwrap());
                }
                Primitive::USize(v) => {
                    *v = Some(u32::from_str(string).unwrap());
                }
                Primitive::U64(v) => {
                    *v = Some(u64::from_str(string).unwrap());
                }
                Primitive::U128(v) => {
                    *v = Some(u128::from_str(string).unwrap());
                }
                Primitive::Felt252(v) => {
                    *v = Some(Felt::from_str(string).unwrap());
                }
                Primitive::ClassHash(v) => {
                    *v = Some(Felt::from_str(string).unwrap());
                }
                Primitive::ContractAddress(v) => {
                    *v = Some(Felt::from_str(string).unwrap());
                }
                Primitive::Bool(v) => {
                    *v = Some(bool::from_str(string).unwrap());
                }
                _ => {
                    return Err(Error::InvalidMessageError("Invalid primitive type".to_string()));
                }
            },
            Ty::ByteArray(s) => {
                s.clone_from(string);
            }
            _ => {
                return Err(Error::InvalidMessageError(format!(
                    "Invalid string type for {}",
                    ty.name()
                )));
            }
        },
    }

    Ok(())
}

// Validates the message model
// and returns the identity and signature
async fn validate_message(
    db: &Sql,
    message: &IndexMap<String, PrimitiveType>,
) -> Result<Ty, Error> {
    let model_name = if let Some(model_name) = message.get("model") {
        if let PrimitiveType::String(model_name) = model_name {
            model_name
        } else {
            return Err(Error::InvalidMessageError("Model name is not a string".to_string()));
        }
    } else {
        return Err(Error::InvalidMessageError("Model name is missing".to_string()));
    };
    let model_selector = get_selector_from_name(model_name).map_err(|e| {
        Error::InvalidMessageError(format!("Failed to get selector from model name: {}", e))
    })?;

    let mut ty = db
        .model(&format!("{:#x}", model_selector))
        .await
        .map_err(|e| Error::InvalidMessageError(format!("Model {} not found: {}", model_name, e)))?
        .schema()
        .await
        .map_err(|e| {
            Error::InvalidMessageError(format!(
                "Failed to get schema for model {}: {}",
                model_name, e
            ))
        })?;

    if let Some(object) = message.get(model_name) {
        parse_value_to_ty(object, &mut ty)?;
    } else {
        return Err(Error::InvalidMessageError("Model is missing".to_string()));
    };

    Ok(ty)
}

fn read_or_create_identity(path: &Path) -> anyhow::Result<identity::Keypair> {
    if path.exists() {
        let bytes = fs::read(path)?;

        info!(target: LOG_TARGET, path = %path.display(), "Using existing identity.");

        return Ok(identity::Keypair::from_protobuf_encoding(&bytes)?); // This only works for ed25519 but that is what we are using.
    }

    let identity = identity::Keypair::generate_ed25519();

    fs::write(path, identity.to_protobuf_encoding()?)?;

    info!(target: LOG_TARGET, path = %path.display(), "Generated new identity.");

    Ok(identity)
}

fn read_or_create_certificate(path: &Path) -> anyhow::Result<Certificate> {
    if path.exists() {
        let pem = fs::read_to_string(path)?;

        info!(target: LOG_TARGET, path = %path.display(), "Using existing certificate.");

        return Ok(Certificate::from_pem(&pem)?);
    }

    let cert = Certificate::generate(&mut rand::thread_rng())?;
    fs::write(path, cert.serialize_pem().as_bytes())?;

    info!(target: LOG_TARGET, path = %path.display(), "Generated new certificate.");

    Ok(cert)
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn test_read_or_create_identity() {
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

    #[test]
    fn test_read_or_create_certificate() {
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
