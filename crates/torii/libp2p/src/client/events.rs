use gossipsub::Event as GossipsubEvent;
use libp2p::{gossipsub, identify, ping};

#[derive(Debug)]
pub(crate) enum ClientEvent {
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
