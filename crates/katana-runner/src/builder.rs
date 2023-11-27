use crate::{binary::KatanaBinary, compiled::KatanaCompiled};
use anyhow::Result;
use starknet::providers::{jsonrpc::HttpTransport, JsonRpcClient};

pub struct KatanaRunnerBuilder(KatanaRunnerConfig);

#[derive(Debug, Clone, Default)]
pub struct KatanaRunnerConfig {
    pub port: Option<u16>,
}

impl KatanaRunnerBuilder {
    pub fn new() -> Self {
        KatanaRunnerBuilder(KatanaRunnerConfig { port: None })
    }

    /// Runs katana on runned port, if not specified, a random free port will be used
    pub fn with_port(mut self, port: u16) -> Self {
        self.0.port = Some(port);
        self
    }

    /// Runs system installed katana version
    pub fn binary(self) -> Result<(KatanaBinary, JsonRpcClient<HttpTransport>)> {
        KatanaBinary::new(self.0)
    }

    /// Compiles and runs katana from source
    pub async fn compiled(self) -> Result<(KatanaCompiled, JsonRpcClient<HttpTransport>)> {
        KatanaCompiled::run(self.0).await
    }
}
