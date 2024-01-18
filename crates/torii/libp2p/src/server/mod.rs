use futures::{stream::StreamExt, Future};
use libp2p::{
    core::multiaddr::Protocol,
    core::Multiaddr,
    gossipsub::{self, IdentTopic}, identify, identity, noise, ping, relay,
    swarm::{NetworkBehaviour, SwarmEvent},
    tcp, yamux, PeerId, Swarm,
};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::time::Duration;
use std::{collections::HashMap, error::Error};

mod events;

use crate::server::events::ServerEvent;

#[derive(NetworkBehaviour)]
#[behaviour(out_event = "ServerEvent")]
pub struct Behaviour {
    relay: relay::Behaviour,
    ping: ping::Behaviour,
    identify: identify::Behaviour,
    gossipsub: gossipsub::Behaviour,
}

pub struct RelayServer {
    swarm: Swarm<Behaviour>,
    rooms: HashMap<String, gossipsub::IdentTopic>,
}

impl RelayServer {
    pub fn new(use_ipv6: Option<bool>, port: u16) -> Result<impl Future<Output = Result<(), Box<dyn Error>>>, Box<dyn Error>> {
        let local_key: identity::Keypair = identity::Keypair::generate_ed25519();

        let mut swarm = libp2p::SwarmBuilder::with_existing_identity(local_key)
            .with_async_std()
            .with_tcp(tcp::Config::default(), noise::Config::new, || yamux::Config::default())?
            .with_quic()
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

        let listen_addr_tcp = Multiaddr::empty()
            .with(match use_ipv6 {
                Some(true) => Protocol::from(Ipv6Addr::UNSPECIFIED),
                _ => Protocol::from(Ipv4Addr::UNSPECIFIED),
            })
            .with(Protocol::Tcp(port));
        swarm.listen_on(listen_addr_tcp)?;

        let listen_addr_quic = Multiaddr::empty()
            .with(match use_ipv6 {
                Some(true) => Protocol::from(Ipv6Addr::UNSPECIFIED),
                _ => Protocol::from(Ipv4Addr::UNSPECIFIED),
            })
            .with(Protocol::Udp(port))
            .with(Protocol::QuicV1);
        swarm.listen_on(listen_addr_quic)?;

        swarm.behaviour_mut().gossipsub.subscribe(&IdentTopic::new("mimi")).unwrap();

        let mut server = Self {
            swarm,
            rooms: HashMap::new(),
        };

        Ok(async move {
            server.run().await;
            Ok(())
        })
    }

    async fn run(&mut self) {
        loop {
            match self.swarm.next().await.expect("Infinite Stream.") {
                SwarmEvent::Behaviour(event) => {
                    match &event {
                        ServerEvent::Gossipsub(gossipsub::Event::Message {
                            propagation_source: peer_id,
                            message_id,
                            message,
                        }) => {
                                let room_name = message.topic.clone();
                                println!("Received message in room {room_name}: {:?}", String::from_utf8_lossy(&message.data));
                                // self.swarm.behaviour_mut().gossipsub.publish(room_name, message.data.clone()).expect("Publishing should work");
                        }
                        ServerEvent::Identify(identify::Event::Received {
                            info: identify::Info { observed_addr, .. },
                            peer_id,
                        }) => {
                            println!("Received identify event from {peer_id:?} with observed address {observed_addr:?}", peer_id = peer_id, observed_addr = observed_addr);
                            self.swarm.add_external_address(observed_addr.clone());
                        }
                        _ => {}
                    }

                    println!("{:?}", event);
                }
                SwarmEvent::NewListenAddr { address, .. } => {
                    println!("Listening on {address:?}");
                }
                _ => {}
            }
        }
    }
}
