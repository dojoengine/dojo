use starknet::core::utils::{cairo_short_string_to_felt, CairoShortStringToFeltError};
use starknet::macros::short_string;

use crate::{Felt, FromStrError};

/// Known chain ids that has been assigned a name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum NamedChainId {
    Mainnet,
    Goerli,
    Sepolia,
}

impl NamedChainId {
    /// `SN_MAIN` in ASCII
    pub const SN_MAIN: Felt = short_string!("SN_MAIN");

    /// `SN_GOERLI` in ASCII
    pub const SN_GOERLI: Felt = short_string!("SN_GOERLI");

    /// `SN_SEPOLIA` in ASCII
    pub const SN_SEPOLIA: Felt = short_string!("SN_SEPOLIA");

    /// Returns the id of the chain. It is the ASCII representation of a predefined string
    /// constants.
    #[inline]
    pub const fn id(&self) -> Felt {
        match self {
            NamedChainId::Mainnet => Self::SN_MAIN,
            NamedChainId::Goerli => Self::SN_GOERLI,
            NamedChainId::Sepolia => Self::SN_SEPOLIA,
        }
    }

    /// Returns the predefined string constant of the chain id.
    #[inline]
    pub const fn name(&self) -> &'static str {
        match self {
            NamedChainId::Mainnet => "SN_MAIN",
            NamedChainId::Goerli => "SN_GOERLI",
            NamedChainId::Sepolia => "SN_SEPOLIA",
        }
    }
}

impl std::fmt::Display for NamedChainId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

/// This `struct` is created by the [`NamedChainId::try_from<u128>`] method.
#[derive(Debug, thiserror::Error)]
#[error("Unknown named chain id {0:#x}")]
pub struct NamedChainTryFromError(Felt);

impl TryFrom<Felt> for NamedChainId {
    type Error = NamedChainTryFromError;
    fn try_from(value: Felt) -> Result<Self, Self::Error> {
        if value == Self::SN_MAIN {
            Ok(Self::Mainnet)
        } else if value == Self::SN_GOERLI {
            Ok(Self::Goerli)
        } else if value == Self::SN_SEPOLIA {
            Ok(Self::Sepolia)
        } else {
            Err(NamedChainTryFromError(value))
        }
    }
}

/// Represents a chain id.
#[derive(Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ChainId {
    /// A chain id with a known chain name.
    Named(NamedChainId),
    Id(Felt),
}

#[derive(Debug, thiserror::Error)]
pub enum ParseChainIdError {
    #[error(transparent)]
    FromStr(#[from] FromStrError),
    #[error(transparent)]
    CairoShortStringToFelt(#[from] CairoShortStringToFeltError),
}

impl ChainId {
    /// Chain id of the Starknet mainnet.
    pub const MAINNET: Self = Self::Named(NamedChainId::Mainnet);
    /// Chain id of the Starknet goerli testnet.
    pub const GOERLI: Self = Self::Named(NamedChainId::Goerli);
    /// Chain id of the Starknet sepolia testnet.
    pub const SEPOLIA: Self = Self::Named(NamedChainId::Sepolia);

    /// Parse a [`ChainId`] from a [`str`].
    ///
    /// If the `str` starts with `0x` it is parsed as a hex string, otherwise it is parsed as a
    /// Cairo short string.
    pub fn parse(s: &str) -> Result<Self, ParseChainIdError> {
        let id =
            if s.starts_with("0x") { Felt::from_hex(s)? } else { cairo_short_string_to_felt(s)? };
        Ok(ChainId::from(id))
    }

    /// Returns the chain id value.
    pub const fn id(&self) -> Felt {
        match self {
            ChainId::Named(name) => name.id(),
            ChainId::Id(id) => *id,
        }
    }
}

impl Default for ChainId {
    fn default() -> Self {
        ChainId::Id(Felt::ZERO)
    }
}

impl std::fmt::Debug for ChainId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChainId::Named(name) => write!(f, "ChainId {{ name: {name}, id: {:#x} }}", name.id()),
            ChainId::Id(id) => write!(f, "ChainId {{ id: {id:#x} }}"),
        }
    }
}

impl std::fmt::Display for ChainId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChainId::Named(id) => write!(f, "{id}"),
            ChainId::Id(id) => write!(f, "{id:#x}"),
        }
    }
}

impl From<Felt> for ChainId {
    fn from(value: Felt) -> Self {
        NamedChainId::try_from(value).map(ChainId::Named).unwrap_or(ChainId::Id(value))
    }
}

impl From<ChainId> for Felt {
    fn from(value: ChainId) -> Self {
        value.id()
    }
}

#[cfg(test)]
mod tests {
    use std::convert::TryFrom;

    use starknet::core::utils::cairo_short_string_to_felt;
    use starknet::macros::felt;

    use super::ChainId;
    use crate::chain::NamedChainId;

    #[test]
    fn named_chain_id() {
        let mainnet_id = cairo_short_string_to_felt("SN_MAIN").unwrap();
        let goerli_id = cairo_short_string_to_felt("SN_GOERLI").unwrap();
        let sepolia_id = cairo_short_string_to_felt("SN_SEPOLIA").unwrap();

        assert_eq!(NamedChainId::Mainnet.id(), mainnet_id);
        assert_eq!(NamedChainId::Goerli.id(), goerli_id);
        assert_eq!(NamedChainId::Sepolia.id(), sepolia_id);

        assert_eq!(NamedChainId::try_from(mainnet_id).unwrap(), NamedChainId::Mainnet);
        assert_eq!(NamedChainId::try_from(goerli_id).unwrap(), NamedChainId::Goerli);
        assert_eq!(NamedChainId::try_from(sepolia_id).unwrap(), NamedChainId::Sepolia);
        assert!(NamedChainId::try_from(felt!("0x1337")).is_err());
    }

    #[test]
    fn chain_id() {
        let mainnet_id = cairo_short_string_to_felt("SN_MAIN").unwrap();
        let goerli_id = cairo_short_string_to_felt("SN_GOERLI").unwrap();
        let sepolia_id = cairo_short_string_to_felt("SN_SEPOLIA").unwrap();

        assert_eq!(ChainId::MAINNET.id(), NamedChainId::Mainnet.id());
        assert_eq!(ChainId::GOERLI.id(), NamedChainId::Goerli.id());
        assert_eq!(ChainId::SEPOLIA.id(), NamedChainId::Sepolia.id());

        assert_eq!(ChainId::from(mainnet_id), ChainId::MAINNET);
        assert_eq!(ChainId::from(goerli_id), ChainId::GOERLI);
        assert_eq!(ChainId::from(sepolia_id), ChainId::SEPOLIA);
        assert_eq!(ChainId::from(felt!("0x1337")), ChainId::Id(felt!("0x1337")));

        assert_eq!(ChainId::MAINNET.to_string(), "SN_MAIN");
        assert_eq!(ChainId::GOERLI.to_string(), "SN_GOERLI");
        assert_eq!(ChainId::SEPOLIA.to_string(), "SN_SEPOLIA");
        assert_eq!(ChainId::Id(felt!("0x1337")).to_string(), "0x1337");
    }

    #[test]
    fn parse_chain_id() {
        let mainnet_id = cairo_short_string_to_felt("SN_MAIN").unwrap();
        let custom_id = cairo_short_string_to_felt("KATANA").unwrap();

        assert_eq!(ChainId::parse("SN_MAIN").unwrap(), ChainId::MAINNET);
        assert_eq!(ChainId::parse("KATANA").unwrap(), ChainId::Id(custom_id));
        assert_eq!(ChainId::parse(&format!("{mainnet_id:#x}")).unwrap(), ChainId::MAINNET);
    }
}
