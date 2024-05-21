use jsonrpsee::core::RpcResult;
use jsonrpsee::proc_macros::rpc;
use katana_primitives::block::BlockIdOrTag;
use katana_rpc_types::trace::TxExecutionInfo;

#[cfg_attr(not(feature = "client"), rpc(server, namespace = "saya"))]
#[cfg_attr(feature = "client", rpc(client, server, namespace = "saya"))]
pub trait SayaApi {
    /// Retrieves a list of transaction execution informations of a given block.
    #[method(name = "getTransactionExecutionsByBlock")]
    async fn transaction_executions_by_block(
        &self,
        block_id: BlockIdOrTag,
    ) -> RpcResult<Vec<TxExecutionInfo>>;
}
