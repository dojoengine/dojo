use jsonrpsee::core::RpcResult;
use jsonrpsee::proc_macros::rpc;
use katana_rpc_types::account::Account;

#[cfg_attr(not(feature = "client"), rpc(server, namespace = "katana"))]
#[cfg_attr(feature = "client", rpc(client, server, namespace = "katana"))]
pub trait KatanaApi {
    #[method(name = "predeployedAccounts")]
    async fn predeployed_accounts(&self) -> RpcResult<Vec<Account>>;
}
