use jsonrpsee::core::RpcResult;
use jsonrpsee::proc_macros::rpc;
use katana_rpc_types::transaction::{TransactionsPage, TransactionsPageCursor};

#[cfg_attr(not(feature = "client"), rpc(server, namespace = "torii"))]
#[cfg_attr(feature = "client", rpc(client, server, namespace = "torii"))]
pub trait ToriiApi {
    #[method(name = "getTransactions")]
    async fn get_transactions(&self, cursor: TransactionsPageCursor)
    -> RpcResult<TransactionsPage>;
}
