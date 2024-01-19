use libp2p::{identify, gossipsub, ping};
use gossipsub::Event as GossipsubEvent;

#[derive(Debug)]
pub enum ClientEvent {
    Gossipsub(GossipsubEvent),
    Identify(identify::Event),
    Ping(ping::Event),
}

impl From<GossipsubEvent> for ClientEvent {
    fn from(event: GossipsubEvent) -> Self {
        Self::Gossipsub(event)
    }
}

impl From<identify::Event> for ClientEvent {
    fn from(event: identify::Event) -> Self {
        Self::Identify(event)
    }
}

impl From<ping::Event> for ClientEvent {
    fn from(event: ping::Event) -> Self {
        Self::Ping(event)
    }
}