use core::fmt;
use std::num::ParseIntError;

use crate::Felt;

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "arbitrary", derive(::arbitrary::Arbitrary))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OrderedEvent {
    pub order: u64,
    pub keys: Vec<Felt>,
    pub data: Vec<Felt>,
}

/// Represents a continuation token for implementing paging in event queries.
///
/// This struct stores the necessary information to resume fetching events
/// from a specific point relative to the given filter passed as parameter to the
/// `starknet_getEvents` API, [EventFilter][starknet::core::types::EventFilter].
///
/// There JSON-RPC specification does not specify the format of the continuation token,
/// so how the node should handle it is implementation specific.
#[derive(PartialEq, Eq, Debug, Clone, Default)]
pub struct ContinuationToken {
    /// The block number to continue from.
    pub block_n: u64,
    /// The transaction number within the block to continue from.
    pub txn_n: u64,
    /// The event number within the transaction to continue from.
    pub event_n: u64,
}

#[derive(PartialEq, Eq, Debug, thiserror::Error)]
pub enum ContinuationTokenError {
    #[error("Invalid data")]
    InvalidToken,
    #[error("Invalid format: {0}")]
    ParseFailed(ParseIntError),
}

impl ContinuationToken {
    pub fn parse(token: &str) -> Result<Self, ContinuationTokenError> {
        let arr: Vec<&str> = token.split(',').collect();
        if arr.len() != 3 {
            return Err(ContinuationTokenError::InvalidToken);
        }
        let block_n =
            u64::from_str_radix(arr[0], 16).map_err(ContinuationTokenError::ParseFailed)?;
        let receipt_n =
            u64::from_str_radix(arr[1], 16).map_err(ContinuationTokenError::ParseFailed)?;
        let event_n =
            u64::from_str_radix(arr[2], 16).map_err(ContinuationTokenError::ParseFailed)?;

        Ok(ContinuationToken { block_n, txn_n: receipt_n, event_n })
    }
}

impl fmt::Display for ContinuationToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:x},{:x},{:x}", self.block_n, self.txn_n, self.event_n)
    }
}

/// Represents a continuation token that can either be a Katana native [`ContinuationToken`] or a
/// continuation token returned by the forked provider.
///
/// This is only used in the `starknet_getEvents` API.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MaybeForkedContinuationToken {
    /// A continuation token returned by the forked provider.
    /// Used to tell Katana to continue fetching events from the forked provider.
    ///
    /// It's a string because there is no a guarantee format.
    Forked(String),
    /// A Katana specific continuation token. Used to tell Katana the next events to fetch is in the
    /// local blocks and not in the forked provider.
    Token(ContinuationToken),
}

impl MaybeForkedContinuationToken {
    /// Parses a continuation token from a string. It can be either a Katana native
    /// [`ContinuationToken`] or a forked token. The forked token is identified by the prefix
    /// `FK_`.
    pub fn parse(value: &str) -> Result<Self, ContinuationTokenError> {
        const FORKED_TOKEN_PREFIX: &str = "FK_";
        if let Some(token) = value.strip_prefix(FORKED_TOKEN_PREFIX) {
            Ok(MaybeForkedContinuationToken::Forked(token.to_string()))
        } else {
            let token = ContinuationToken::parse(value)?;
            Ok(MaybeForkedContinuationToken::Token(token))
        }
    }

    /// Tries to convert the continuation token to a Katana native [`ContinuationToken`]. `None` if
    /// the continuation token is a forked token.
    pub fn to_token(self) -> Option<ContinuationToken> {
        match self {
            MaybeForkedContinuationToken::Token(token) => Some(token),
            _ => None,
        }
    }
}

impl std::fmt::Display for MaybeForkedContinuationToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MaybeForkedContinuationToken::Token(token) => write!(f, "{token}"),
            MaybeForkedContinuationToken::Forked(token) => write!(f, "FK_{token}"),
        }
    }
}

#[cfg(test)]
mod test {
    use assert_matches::assert_matches;

    use super::*;

    #[test]
    fn to_string_works() {
        fn helper(block_n: u64, txn_n: u64, event_n: u64) -> String {
            ContinuationToken { block_n, txn_n, event_n }.to_string()
        }

        assert_eq!(helper(0, 0, 0), "0,0,0");
        assert_eq!(helper(30, 255, 4), "1e,ff,4");
    }

    #[test]
    fn parse_works() {
        fn helper(token: &str) -> ContinuationToken {
            ContinuationToken::parse(token).unwrap()
        }
        assert_eq!(helper("0,0,0"), ContinuationToken { block_n: 0, txn_n: 0, event_n: 0 });
        assert_eq!(helper("1e,ff,4"), ContinuationToken { block_n: 30, txn_n: 255, event_n: 4 });
    }

    #[test]
    fn parse_should_fail() {
        assert_eq!(
            ContinuationToken::parse("100").unwrap_err(),
            ContinuationTokenError::InvalidToken
        );
        assert_eq!(
            ContinuationToken::parse("0,").unwrap_err(),
            ContinuationTokenError::InvalidToken
        );
        assert_eq!(
            ContinuationToken::parse("0,0").unwrap_err(),
            ContinuationTokenError::InvalidToken
        );
    }

    #[test]
    fn parse_u64_should_fail() {
        matches!(
            ContinuationToken::parse("2y,100,4").unwrap_err(),
            ContinuationTokenError::ParseFailed(_)
        );
        matches!(
            ContinuationToken::parse("30,255g,4").unwrap_err(),
            ContinuationTokenError::ParseFailed(_)
        );
        matches!(
            ContinuationToken::parse("244,1,fv").unwrap_err(),
            ContinuationTokenError::ParseFailed(_)
        );
    }

    #[test]
    fn parse_forked_token_works() {
        let forked_token = "FK_test_token";
        let parsed = MaybeForkedContinuationToken::parse(forked_token).unwrap();
        assert_matches!(parsed, MaybeForkedContinuationToken::Forked(s) => {
            assert_eq!(s, "test_token")
        });

        let regular_token = "1e,ff,4";
        let parsed = MaybeForkedContinuationToken::parse(regular_token).unwrap();
        assert_matches!(parsed, MaybeForkedContinuationToken::Token(t) => {
            assert_eq!(t.block_n, 30);
            assert_eq!(t.txn_n, 255);
            assert_eq!(t.event_n, 4);
        });
    }
}
