use dojo_test_utils::sequencer::{get_default_test_config, TestSequencer};
use jsonrpsee::http_client::HttpClientBuilder;
use katana_node::config::SequencingConfig;
use katana_rpc_api::katana::KatanaApiClient;

#[tokio::test]
async fn default_fee_tokens() {
    let sequencer =
        TestSequencer::start(get_default_test_config(SequencingConfig::default())).await;
    let chainspec = &sequencer.backend().chain_spec;

    let client = HttpClientBuilder::default().build(sequencer.url()).unwrap();

    let fee_tokens = client.fee_tokens().await.unwrap();

    assert_eq!(fee_tokens[0].name, "ETH");
    assert_eq!(fee_tokens[0].address, chainspec.fee_contracts.eth);
    assert_eq!(fee_tokens[1].name, "STRK");
    assert_eq!(fee_tokens[1].address, chainspec.fee_contracts.strk);
}
