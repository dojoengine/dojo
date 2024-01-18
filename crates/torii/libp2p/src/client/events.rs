use libp2p::gossipsub::Event as GossipsubEvent;

#[derive(Debug)]
pub enum ClientEvent {
    Gossipsub(GossipsubEvent),
}

impl From<GossipsubEvent> for ClientEvent {
    fn from(event: GossipsubEvent) -> Self {
        Self::Gossipsub(event)
    }
}