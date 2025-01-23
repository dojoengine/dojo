use std::str::FromStr;

use starknet::macros::short_string;

use crate::cairo::{ShortString, ShortStringFromFeltError, ShortStringTryFromStrError};
use crate::{Felt, FromStrError};

/// An chain id that is not necessarily a valid ASCII string and thus
/// cannot be displayed as a formatted string (eg., `SN_MAIN`).
pub type RawChainId = Felt;

#[derive(Debug, thiserror::Error)]
pub enum NamedIdError {
    #[error(transparent)]
    InvalidShortString(#[from] ShortStringTryFromStrError),
}

/// Chain id with a canonical human-readable name.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "arbitrary", derive(::arbitrary::Arbitrary))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NamedChainId(Felt);

//////////////////////////////////////////////////////////////
// 	NamedChainId implementations
//////////////////////////////////////////////////////////////

impl NamedChainId {
    /// The official chain id for Starknet mainnet.
    pub const SN_MAIN: Felt = short_string!("SN_MAIN");
    /// The official chain id for (now removed) Starknet goerli.
    pub const SN_GOERLI: Felt = short_string!("SN_GOERLI");
    /// The official chain id for Starknet sepolia.
    pub const SN_SEPOLIA: Felt = short_string!("SN_SEPOLIA");

    pub fn new(name: &str) -> Result<Self, NamedIdError> {
        Ok(Self(ShortString::from_str(name)?.into()))
    }

    /// Returns the id of the chain. It is the ASCII representation of a predefined string
    /// constants.
    #[inline]
    pub fn raw(&self) -> RawChainId {
        self.0
    }

    /// Returns the predefined string constant of the chain id.
    #[inline]
    pub fn name(&self) -> ShortString {
        // safe to just unwrap here because we already checked its validity upon creation
        ShortString::try_from(self.0).unwrap()
    }
}

impl std::fmt::Display for NamedChainId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

impl TryFrom<Felt> for NamedChainId {
    type Error = ShortStringFromFeltError;

    fn try_from(value: Felt) -> Result<Self, Self::Error> {
        Ok(Self(ShortString::try_from(value)?))
    }
}

impl From<NamedChainId> for Felt {
    fn from(value: NamedChainId) -> Self {
        value.raw()
    }
}

/// Represents a chain id.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "arbitrary", derive(::arbitrary::Arbitrary))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ChainId {
    Raw(RawChainId),
    Named(NamedChainId),
}

//////////////////////////////////////////////////////////////
// 	ChainId implementations
//////////////////////////////////////////////////////////////

impl ChainId {
    /// Parse a [`ChainId`] from a [`str`].
    ///
    /// If the `str` starts with `0x` it is parsed as a hex string, otherwise it is parsed as a
    /// Cairo short string.
    pub fn parse(s: &str) -> Result<Self, ParseChainIdError> {
        if s.starts_with("0x") {
            Ok(ChainId::Raw(Felt::from_hex(s)?))
        } else {
            Ok(ChainId::Named(NamedChainId::new(s)?))
        }
    }

    /// Returns the raw chain id value.
    pub fn raw(&self) -> RawChainId {
        match self {
            ChainId::Raw(id) => *id,
            ChainId::Named(name) => name.raw(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ParseChainIdError {
    #[error(transparent)]
    FromStr(#[from] FromStrError),
    #[error(transparent)]
    NamedId(#[from] NamedIdError),
}

impl std::fmt::Display for ChainId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChainId::Raw(id) => write!(f, "{id:#x}"),
            ChainId::Named(id) => write!(f, "{}", id.name()),
        }
    }
}

impl From<Felt> for ChainId {
    fn from(value: Felt) -> Self {
        NamedChainId::try_from(value).map(ChainId::Named).unwrap_or(ChainId::Raw(value))
    }
}

impl Default for ChainId {
    fn default() -> Self {
        Self::Raw(RawChainId::ZERO)
    }
}

impl From<ChainId> for Felt {
    fn from(value: ChainId) -> Self {
        value.raw()
    }
}

#[cfg(test)]
mod tests {
    use std::convert::TryFrom;

    use starknet::macros::short_string;

    use super::ChainId;
    use crate::chain::NamedChainId;
    use crate::{felt, Felt};

    #[test]
    fn named_chain_id() {
        let id = NamedChainId::new("KATANA").unwrap();
        let felt: Felt = id.clone().into();
        let id2 = NamedChainId::try_from(felt).unwrap();
        assert_eq!(id, id2);
    }

    #[rstest::rstest]
    #[case("😜")]
    #[case("漢字")]
    #[case("ˆåß∂ßå∂åß")]
    fn invalid_named_chain_id(#[case] invalid_string: &str) {
        assert!(dbg!(NamedChainId::new(invalid_string)).is_err());
    }

    #[rstest::rstest]
    #[case("0x1337", felt!("0x1337"))]
    #[case("KATANA", short_string!("KATANA"))]
    #[case("0xdeadbeef", felt!("0xdeadbeef"))]
    #[case("SN_MAIN", short_string!("SN_MAIN"))]
    #[case("FARTCHAIN", short_string!("FARTCHAIN"))]
    #[case("SN_SEPOLIA", short_string!("SN_SEPOLIA"))]
    fn parse_chain_id(#[case] string: &str, #[case] felt: Felt) {
        let id = ChainId::parse(string).unwrap();
        assert_eq!(id.raw(), felt);
    }
}
