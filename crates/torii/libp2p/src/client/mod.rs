use futures::StreamExt;
use libp2p::{
    identity,
    core::{multiaddr::Protocol},
    gossipsub::{self, IdentTopic, MessageAuthenticity},
    noise,
    swarm::{NetworkBehaviour, Swarm, SwarmEvent},
    tcp, yamux, PeerId,
};
use std::{collections::HashMap, error::Error};

mod events;
use crate::client::events::ClientEvent;

#[derive(NetworkBehaviour)]
#[behaviour(out_event = "ClientEvent")]
pub struct Behaviour {
    gossipsub: gossipsub::Behaviour,
    // Add other behaviours if needed
}

pub struct RelayClient {
    swarm: Swarm<Behaviour>,
}

impl RelayClient {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let local_key = identity::Keypair::generate_ed25519();
        let peer_id = PeerId::from(local_key.public());

        let mut swarm = libp2p::SwarmBuilder::with_existing_identity(local_key)
            .with_async_std()
            .with_tcp(tcp::Config::default(), noise::Config::new, || yamux::Config::default())?
            .with_behaviour(|key| {
                let gossipsub_config = gossipsub::ConfigBuilder::default()
                    .heartbeat_interval(std::time::Duration::from_secs(10))
                    .build()
                    .expect("Gossipsub configuration should be valid");

                Behaviour {
                    gossipsub: gossipsub::Behaviour::new(
                        gossipsub::MessageAuthenticity::Signed(key.clone()),
                        gossipsub_config,
                    )
                    .expect("Valid gossipsub behaviour")
                }
            })?
            .build();

        Ok(Self { swarm })
    }

    pub async fn run(&mut self) {
        while let Some(event) = self.swarm.next().await {
            match event {
                SwarmEvent::Behaviour(ClientEvent::Gossipsub(message)) => {
                    // checxk if event type Message
                    match message {
                        gossipsub::Event::Message { propagation_source, message_id, message } => {
                            println!("Received: {:?}", String::from_utf8_lossy(&message.data));
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }

    pub fn join_room(&mut self, topic: IdentTopic) {
        self.swarm.behaviour_mut().gossipsub.subscribe(&topic).unwrap();
    }

    pub fn leave_room(&mut self, topic: IdentTopic) {
        self.swarm.behaviour_mut().gossipsub.unsubscribe(&topic).unwrap();
    }

    pub fn send_message(&mut self, topic: IdentTopic, message: &str) {
        self.swarm.behaviour_mut().gossipsub.publish(topic, message.as_bytes()).unwrap();
    }
}
