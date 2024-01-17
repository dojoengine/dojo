use libp2p::{gossipsub, mdns, noise, swarm::NetworkBehaviour, swarm::SwarmEvent, tcp, yamux};

use std::collections::HashMap;

#[derive(NetworkBehaviour)]
pub struct Behaviour {
    kademlia: Kademlia<MemoryStore>,
    rooms: HashMap<String, Vec<PeerId>>,
}

impl NetworkBehaviourEventProcess<KademliaEvent> for Behaviour {
    fn inject_event(&mut self, event: KademliaEvent) {
        match event {
            KademliaEvent::UnroutedQuery { .. } => {}
            KademliaEvent::RoutableQuery { .. } => {}
            KademliaEvent::RoutingUpdated { .. } => {}
            KademliaEvent::RoutingUpdateFailed { .. } => {}
            KademliaEvent::QueryResult { .. } => {}
        }
    }
}

pub fn new() -> Swarm<Behaviour> {
    let local_key = identity::Keypair::generate_ed25519();
    let local_peer_id = PeerId::from(local_key.public());

    let transport = DnsConfig::new(
        TcpConfig::new()
            .upgrade(libp2p::core::upgrade::Version::V1)
            .authenticate(NoiseConfig::xx(Keypair::new().into_authentic(&local_key)).unwrap())
            .multiplex(libp2p::yamux::Config::default())
            .boxed(),
    );

    let store = MemoryStore::new(local_peer_id.clone());
    let kademlia = Kademlia::with_config(local_peer_id.clone(), store, KademliaConfig::default());

    let behaviour = Behaviour { kademlia, rooms: HashMap::new() };

    SwarmBuilder::new(transport, behaviour, local_peer_id)
        .executor(Box::new(|fut| {
            async_std::task::spawn(fut);
        }))
        .build()
}
