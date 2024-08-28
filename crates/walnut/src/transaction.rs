use scarb_ui::Ui;
use starknet::core::types::Felt;
use url::Url;
use urlencoding::encode;

pub fn walnut_debug_transaction(ui: &Ui, rpc_url: &Url, transaction_hash: &Felt) {
    if rpc_url.host_str() != Some("localhost") && rpc_url.host_str() != Some("127.0.0.1") {
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
