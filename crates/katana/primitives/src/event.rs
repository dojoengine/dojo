use core::fmt;
use std::num::ParseIntError;

use crate::FieldElement;

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OrderedEvent {
    pub order: u64,
    pub keys: Vec<FieldElement>,
    pub data: Vec<FieldElement>,
}

/// Represents a continuation token for implementing paging in event queries.
///
/// This struct stores the necessary information to resume fetching events
/// from a specific point relative to the given filter passed as parameter to the
/// `starknet_getEvents` API, [EventFilter][starknet::core::types::EventFilter].
///
/// There JSON-RPC specification does not specify the format of the continuation token,
/// so how the node should handle it is implementation specific.
#[derive(PartialEq, Eq, Debug, Default)]
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
    #[error("Missing block number. Expected format: block_n,txn_n,event_n")]
    MissingBlock,
    #[error("Missing transaction number. Expected format: block_n,txn_n,event_n")]
    MissingTxn,
    #[error("Missing event number. Expected format: block_n,txn_n,event_n")]
    MissingEvent,
    #[error("Invalid data")]
    InvalidToken,
    #[error("Invalid value: {0}")]
    ParseFailed(#[from] ParseIntError),
}

impl ContinuationToken {
    pub fn parse(token: &str) -> Result<Self, ContinuationTokenError> {
        let mut parts = token.split(',');

        if parts.clone().count() > 3 {
            return Err(ContinuationTokenError::InvalidToken);
        }

        macro_rules! part {
            ($error:expr) => {{
                let part = parts.next().ok_or($error)?;
                if part.is_empty() {
                    return Err($error);
                }
                u64::from_str_radix(part, 16)?
            }};
        }

        let block_n = part!(ContinuationTokenError::MissingBlock);
        let txn_n = part!(ContinuationTokenError::MissingTxn);
        let event_n = part!(ContinuationTokenError::MissingEvent);

        Ok(ContinuationToken { block_n, txn_n, event_n })
    }
}
impl fmt::Display for ContinuationToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:x},{:x},{:x}", self.block_n, self.txn_n, self.event_n)
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
        assert_matches!(
            ContinuationToken::parse("100").unwrap_err(),
            ContinuationTokenError::MissingTxn
        );
        assert_matches!(
            ContinuationToken::parse("0,").unwrap_err(),
            ContinuationTokenError::MissingTxn
        );
        assert_matches!(
            ContinuationToken::parse("0,0").unwrap_err(),
            ContinuationTokenError::MissingEvent
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
}
