use libp2p::{
    core::{identity, Multiaddr},
    gossipsub::{GossipsubConfig, GossipsubMessage, IdentTopic, MessageAuthenticity},
    noise::{Keypair, NoiseConfig, X25519Spec},
    swarm::{Swarm, SwarmEvent},
    tcp::TcpConfig, yamux::YamuxConfig, mplex, SwarmBuilder,
};
use async_std::{io, task};
use std::error::Error;

async fn run_client() -> Result<(), Box<dyn Error>> {
    let id_keys = identity::Keypair::generate_ed25519();
    let noise_keys = Keypair::<X25519Spec>::new().into_authentic(&id_keys)?;

    let transport = TcpConfig::new()
        .upgrade(libp2p::core::upgrade::Version::V1)
        .authenticate(NoiseConfig::xx(noise_keys).into_authenticated())
        .multiplex(libp2p::core::upgrade::SelectUpgrade::new(YamuxConfig::default(), mplex::MplexConfig::new()))
        .boxed();

    let gossipsub_config = GossipsubConfig::default();
    let mut gossipsub = libp2p::gossipsub::Gossipsub::new(MessageAuthenticity::Signed(id_keys), gossipsub_config).unwrap();

    let global_topic = IdentTopic::new("global");
    gossipsub.subscribe(&global_topic).unwrap();

    let mut swarm = {
        let peer_id = PeerId::from(id_keys.public());
        SwarmBuilder::new(transport, gossipsub, peer_id)
            .executor(Box::new(|fut| { async_std::task::spawn(fut); }))
            .build()
    };

    task::spawn(async move {
        loop {
            match swarm.next().await {
                Some(SwarmEvent::Behaviour(libp2p::gossipsub::GossipsubEvent::Message {
                    message,
                    ..
                })) => {
                    println!("Received: {:?}", String::from_utf8_lossy(&message.data));
                }
                _ => {}
            }
        }
    });

    let mut stdin = io::BufReader::new(io::stdin()).lines();

    while let Some(line) = stdin.next().await {
        let line = line?;
        swarm.behaviour_mut().publish(&global_topic, line.as_bytes()).unwrap();
    }

    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    task::block_on(run_client())
}
