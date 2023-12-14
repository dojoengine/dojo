use std::sync::Arc;

use katana_primitives::block::BlockHashOrNumber;
use katana_provider::providers::fork::ForkedProvider;
use katana_provider::providers::in_memory::InMemoryProvider;
use katana_provider::BlockchainProvider;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::JsonRpcClient;
use url::Url;

#[rstest::fixture]
pub fn in_memory_provider() -> BlockchainProvider<InMemoryProvider> {
    BlockchainProvider::new(InMemoryProvider::new())
}

#[rstest::fixture]
pub fn fork_provider(
    #[default("http://localhost:5050")] rpc: &str,
    #[default(0)] block_num: u64,
) -> BlockchainProvider<ForkedProvider> {
    let rpc_provider = JsonRpcClient::new(HttpTransport::new(Url::parse(rpc).unwrap()));
    let provider = ForkedProvider::new(Arc::new(rpc_provider), BlockHashOrNumber::Num(block_num));
    BlockchainProvider::new(provider)
}
