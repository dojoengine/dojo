use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::JsonRpcClient;

#[derive(Debug)]
pub struct ForkedClient {
    pub client: JsonRpcClient<HttpTransport>,
}
