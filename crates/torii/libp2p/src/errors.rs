use std::{convert::Infallible, io};
use libp2p::{
    Multiaddr, gossipsub::{
        SubscriptionError, PublishError
    },
    noise
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error(transparent)]
    MultiaddrParseError(#[from] libp2p::core::multiaddr::Error),

    #[error(transparent)]
    NoiseUpgradeError(#[from] noise::Error),

    #[error(transparent)]
    DialError(#[from] libp2p::swarm::DialError),

    // accept any error
    #[error(transparent)]
    BehaviourError(#[from] Infallible), 

    #[error(transparent)]
    GossipConfigError(#[from] libp2p::gossipsub::ConfigBuilderError),

    #[error(transparent)]
    TransportError(#[from] libp2p::TransportError<io::Error>),

    #[error(transparent)]
    SubscriptionError(#[from] SubscriptionError),

    #[error(transparent)]
    PublishError(#[from] PublishError),
}