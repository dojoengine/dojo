//! Code adapted from Paradigm's [`reth`](https://github.com/paradigmxyz/reth/tree/main/crates/metrics/metrics-derive) [Metrics] derive macro implementation.

use syn::{Attribute, DeriveInput, Field};

pub(crate) trait WithAttrs {
    fn attrs(&self) -> &[Attribute];
}

impl WithAttrs for DeriveInput {
    fn attrs(&self) -> &[Attribute] {
        &self.attrs
    }
}

impl WithAttrs for Field {
    fn attrs(&self) -> &[Attribute] {
        &self.attrs
    }
}
