use jsonrpsee::core::RpcResult;
use jsonrpsee::proc_macros::rpc;
use katana_rpc_types::transaction::{TransactionsExecutionsPage, TransactionsPageCursor};

#[cfg_attr(not(feature = "client"), rpc(server, namespace = "saya"))]
#[cfg_attr(feature = "client", rpc(client, server, namespace = "saya"))]
pub trait SayaApi {
    /// Fetches the transaction execution info for all the transactions in the
    /// given block.
    ///
    /// The returned [`TransactionsPageCursor`] will always have the same block number
    /// as the cursor given to the function. Only the transaction index may change.
    ///
    /// # Arguments
    ///
    /// * `cursor` - The cursor used to fetch transactions executions info from the block.
    #[method(name = "getTransactionsExecutions")]
    async fn get_transactions_executions(
        &self,
        cursor: TransactionsPageCursor,
    ) -> RpcResult<TransactionsExecutionsPage>;
}
