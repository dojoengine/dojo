use starknet::core::types::Felt;
use url::Url;
use urlencoding::encode;

use crate::{Error, WALNUT_APP_URL};

pub fn walnut_debug_transaction(rpc_url: &Url, transaction_hash: &Felt) -> Result<Url, Error> {
    // Check if the RPC URL is not localhost
    if rpc_url.host_str() != Some("localhost") && rpc_url.host_str() != Some("127.0.0.1") {
        let mut url = Url::parse(WALNUT_APP_URL)?;

        url.path_segments_mut().unwrap().push("transactions");
        url.query_pairs_mut()
            .append_pair("rpcUrl", &encode(rpc_url.as_str()))
            .append_pair("txHash", &format!("{transaction_hash:#066x}"));

        Ok(url)
    } else {
        Err(Error::UnsupportedNetwork)
    }
}

#[cfg(test)]
mod tests {

    use starknet::macros::felt;

    use super::*;

    #[test]
    fn test_walnut_debug_transaction_hosted() {
        let rpc_url = Url::parse("https://example.com").unwrap();
        let transaction_hash = felt!("0x1234");

        let result = walnut_debug_transaction(&rpc_url, &transaction_hash);

        assert!(result.is_ok());
        let debug_url = result.unwrap();
        assert!(debug_url.as_str().starts_with(WALNUT_APP_URL));
        assert!(debug_url.as_str().contains("rpcUrl=https%253A%252F%252Fexample.com"));
        assert!(
            debug_url.as_str().contains(
                "txHash=0x0000000000000000000000000000000000000000000000000000000000001234"
            )
        );
    }

    #[test]
    fn test_walnut_debug_transaction_localhost() {
        let rpc_url = Url::parse("http://localhost:5050").unwrap();
        let transaction_hash = felt!("0x1234");

        let result = walnut_debug_transaction(&rpc_url, &transaction_hash);

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::UnsupportedNetwork));
    }
}
