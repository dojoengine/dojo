use jsonrpsee::core::RpcResult;
use jsonrpsee::proc_macros::rpc;
use katana_rpc_types::fee::FeeToken;

#[cfg_attr(not(feature = "client"), rpc(server, namespace = "katana"))]
#[cfg_attr(feature = "client", rpc(client, server, namespace = "katana"))]
pub trait KatanaApi {
    /// Returns a list of fee tokens supported by the chain.
    #[method(name = "feeTokens")]
    async fn fee_tokens(&self) -> RpcResult<Vec<FeeToken>>;
}
