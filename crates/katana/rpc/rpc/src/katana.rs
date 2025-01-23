use std::sync::Arc;

use jsonrpsee::core::{async_trait, RpcResult};
use katana_core::backend::Backend;
use katana_executor::ExecutorFactory;
pub use katana_rpc_api::katana::KatanaApiServer;
use katana_rpc_types::fee::FeeToken;

#[allow(missing_debug_implementations)]
pub struct KatanaApi<EF: ExecutorFactory> {
    backend: Arc<Backend<EF>>,
}

impl<EF: ExecutorFactory> KatanaApi<EF> {
    pub fn new(backend: Arc<Backend<EF>>) -> Self {
        Self { backend }
    }
}

#[async_trait]
impl<EF: ExecutorFactory> KatanaApiServer for KatanaApi<EF> {
    async fn fee_tokens(&self) -> RpcResult<Vec<FeeToken>> {
        Ok(vec![
            FeeToken {
                name: "Ether".to_string(),
                address: self.backend.chain_spec.fee_contracts.eth,
            },
            FeeToken {
                name: "Starknet Token".to_string(),
                address: self.backend.chain_spec.fee_contracts.strk,
            },
        ])
    }
}
