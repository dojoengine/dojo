use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::JsonRpcClient;
use url::Url;

#[derive(Debug)]
pub struct ForkedClient {
    pub(crate) inner: JsonRpcClient<HttpTransport>,
}

impl ForkedClient {
    pub fn new(url: Url) -> Self {
        Self { inner: JsonRpcClient::new(HttpTransport::new(url)) }
    }
}
