use libp2p::{
    core::{upgrade, Multiaddr, PeerId, Transport},
    futures::StreamExt,
    gossipsub::{
        Gossipsub, GossipsubConfig, GossipsubEvent, GossipsubMessage, IdentTopic, MessageAuthenticity, ValidationMode,
    },
    identity, mplex, noise::{Keypair, NoiseConfig, X25519Spec},
    swarm::{Swarm, SwarmBuilder, SwarmEvent},
    tcp::TcpConfig, yamux::YamuxConfig,
};
use std::collections::HashMap;
use std::error::Error;

pub struct ChatServer {
    swarm: Swarm<Gossipsub>,
    rooms: HashMap<String, IdentTopic>,
}

impl ChatServer {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let id_keys = identity::Keypair::generate_ed25519();
        let peer_id = PeerId::from(id_keys.public());

        let noise_keys = Keypair::<X25519Spec>::new().into_authentic(&id_keys)?;

        let transport = TcpConfig::new()
            .upgrade(upgrade::Version::V1)
            .authenticate(NoiseConfig::xx(noise_keys).into_authenticated())
            .multiplex(upgrade::SelectUpgrade::new(YamuxConfig::default(), mplex::MplexConfig::new()))
            .boxed();

        let gossipsub_config = GossipsubConfig::default();
        let mut gossipsub = Gossipsub::new(MessageAuthenticity::Signed(id_keys), gossipsub_config).unwrap();

        gossipsub.subscribe(&IdentTopic::new("global")).unwrap();

        let swarm = SwarmBuilder::new(transport, gossipsub, peer_id)
            .executor(Box::new(|fut| { async_std::task::spawn(fut); }))
            .build();

        Ok(Self {
            swarm,
            rooms: HashMap::new(),
        })
    }

    pub async fn run(&mut self) {
        loop {
            match self.swarm.next().await {
                Some(SwarmEvent::Behaviour(GossipsubEvent::Message {
                    propagation_source: _peer_id,
                    message_id: _id,
                    message,
                })) => {
                    self.handle_message(message).await;
                }
                _ => {}
            }
        }
    }

    async fn handle_message(&mut self, message: GossipsubMessage) {
        if let Ok(text) = String::from_utf8(message.data.clone()) {
            if text.starts_with("/join ") {
                let room_name = text[6..].to_string();
                let topic = IdentTopic::new(&room_name);

                if !self.rooms.contains_key(&room_name) {
                    self.swarm.behaviour_mut().subscribe(&topic).unwrap();
                    self.rooms.insert(room_name.clone(), topic.clone());
                }

                // Relay join message
                self.swarm.behaviour_mut().publish(topic, format!("{} has joined the room.", message.source).as_bytes()).unwrap();
            } else {
                // Relay message to all subscribed rooms
                for topic in self.rooms.values() {
                    self.swarm.behaviour_mut().publish(topic.clone(), message.data.clone()).unwrap();
                }
            }
        }
    }
}
