use scarb_ui::Ui;
use starknet::core::types::Felt;
use url::Url;
use urlencoding::encode;

/// Prints a URL to the Walnut page for debugging the transaction.
/// Only supported on hosted networks (non-localhost).
pub fn walnut_debug_transaction(ui: &Ui, rpc_url: &Url, transaction_hash: &Felt) {
    // Check if the RPC URL is not localhost
    if rpc_url.host_str() != Some("localhost") && rpc_url.host_str() != Some("127.0.0.1") {
        // Encode the RPC URL
        let encoded_rpc_url = encode(rpc_url.as_str());
        ui.print(format!(
            "Debug transaction with Walnut: https://app.walnut.dev/transactions?rpcUrl={}&txHash={:#066x}",
            encoded_rpc_url,
            transaction_hash
        ));
    } else {
        ui.print("Debugging transactions with Walnut is only supported on hosted networks");
    }
}
