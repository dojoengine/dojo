use libp2p::gossipsub::Event as GossipsubEvent;
use libp2p::identify::Event as IdentifyEvent;
use libp2p::ping::Event as PingEvent;
use libp2p::relay::Event as RelayEvent;

#[derive(Debug)]
pub enum ServerEvent {
    Identify(IdentifyEvent),
    Ping(PingEvent),
    Relay(RelayEvent),
    Gossipsub(GossipsubEvent),
}

impl From<IdentifyEvent> for ServerEvent {
    fn from(event: IdentifyEvent) -> Self {
        Self::Identify(event)
    }
}

impl From<PingEvent> for ServerEvent {
    fn from(event: PingEvent) -> Self {
        Self::Ping(event)
    }
}

impl From<RelayEvent> for ServerEvent {
    fn from(event: RelayEvent) -> Self {
        Self::Relay(event)
    }
}

impl From<GossipsubEvent> for ServerEvent {
    fn from(event: GossipsubEvent) -> Self {
        Self::Gossipsub(event)
    }
}
