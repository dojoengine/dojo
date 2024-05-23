use jsonrpsee::core::RpcResult;
use jsonrpsee::proc_macros::rpc;
use katana_primitives::block::BlockIdOrTag;
use katana_rpc_types::trace::TxExecutionInfo;
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

    /// Retrieves a list of transaction execution informations of a given block.
    #[method(name = "getTransactionExecutionsByBlock")]
    async fn transaction_executions_by_block(
        &self,
        block_id: BlockIdOrTag,
    ) -> RpcResult<Vec<TxExecutionInfo>>;
}
