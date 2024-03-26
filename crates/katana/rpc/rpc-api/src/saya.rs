use jsonrpsee::core::RpcResult;
use jsonrpsee::proc_macros::rpc;
use katana_rpc_types::transaction::{TransactionsExecutionsPage, TransactionsPageCursor};

#[cfg_attr(not(feature = "client"), rpc(server, namespace = "saya"))]
#[cfg_attr(feature = "client", rpc(client, server, namespace = "saya"))]
pub trait SayaApi {
    /// Fetches the transaction execution info for all the transactions in the
    /// given block.
    ///
    /// # Arguments
    ///
    /// * `block_number` - The block number to get executions from.
    /// * `chunk_size` - The maximum number of transaction execution that should be returned.
    #[method(name = "getTransactionsExecutions")]
    async fn get_transactions_executions(
        &self,
        cursor: TransactionsPageCursor,
    ) -> RpcResult<TransactionsExecutionsPage>;
}
