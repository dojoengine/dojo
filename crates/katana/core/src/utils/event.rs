use core::fmt;
use std::num::ParseIntError;

#[derive(PartialEq, Eq, Debug, Default)]
pub struct ContinuationToken {
    pub block_n: u64,
    pub txn_n: u64,
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
    pub fn parse(token: String) -> Result<Self, ContinuationTokenError> {
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

#[cfg(test)]
mod test {
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
            ContinuationToken::parse(token.to_owned()).unwrap()
        }
        assert_eq!(helper("0,0,0"), ContinuationToken { block_n: 0, txn_n: 0, event_n: 0 });
        assert_eq!(helper("1e,ff,4"), ContinuationToken { block_n: 30, txn_n: 255, event_n: 4 });
    }

    #[test]
    fn parse_should_fail() {
        assert_eq!(
            ContinuationToken::parse("100".to_owned()).unwrap_err(),
            ContinuationTokenError::InvalidToken
        );
        assert_eq!(
            ContinuationToken::parse("0,".to_owned()).unwrap_err(),
            ContinuationTokenError::InvalidToken
        );
        assert_eq!(
            ContinuationToken::parse("0,0".to_owned()).unwrap_err(),
            ContinuationTokenError::InvalidToken
        );
    }

    #[test]
    fn parse_u64_should_fail() {
        matches!(
            ContinuationToken::parse("2y,100,4".to_owned()).unwrap_err(),
            ContinuationTokenError::ParseFailed(_)
        );
        matches!(
            ContinuationToken::parse("30,255g,4".to_owned()).unwrap_err(),
            ContinuationTokenError::ParseFailed(_)
        );
        matches!(
            ContinuationToken::parse("244,1,fv".to_owned()).unwrap_err(),
            ContinuationTokenError::ParseFailed(_)
        );
    }
}
