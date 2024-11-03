//! A descriptor for a resource, which can be an address, a name, or a tag.

use std::fmt;
use std::str::FromStr;

use anyhow::Result;
use dojo_world::contracts::naming;
use starknet::core::types::Felt;

#[derive(Debug, Clone)]
pub enum ResourceDescriptor {
    Address(Felt),
    Name(String),
    Tag(String),
}

impl ResourceDescriptor {
    /// Parse a resource descriptor from a string.
    ///
    /// The string is only considered an address if it starts with "0x".
    /// A tag is when the string is a valid tag having exactly one `-`.
    /// Otherwise, it is considered a name.
    pub fn from_string(s: &str) -> Result<Self> {
        if s.starts_with("0x") {
            Ok(ResourceDescriptor::Address(Felt::from_str(s)?))
        } else if naming::is_valid_tag(s) {
            Ok(ResourceDescriptor::Tag(s.to_string()))
        } else {
            Ok(ResourceDescriptor::Name(s.to_string()))
        }
    }

    /// Ensure the resource descriptor has a namespace.
    pub fn ensure_namespace(self, default_namespace: &str) -> Self {
        match self {
            ResourceDescriptor::Tag(tag) => {
                ResourceDescriptor::Tag(naming::ensure_namespace(&tag, default_namespace))
            }
            ResourceDescriptor::Name(name) => {
                ResourceDescriptor::Tag(naming::ensure_namespace(&name, default_namespace))
            }
            _ => self,
        }
    }
}

impl FromStr for ResourceDescriptor {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        ResourceDescriptor::from_string(s)
    }
}

impl fmt::Display for ResourceDescriptor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ResourceDescriptor::Address(address) => write!(f, "{:#066x}", address),
            ResourceDescriptor::Name(name) => write!(f, "{}", name),
            ResourceDescriptor::Tag(tag) => write!(f, "{}", tag),
        }
    }
}
