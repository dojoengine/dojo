use account_sdk::account::outside_execution::OutsideExecution;
use jsonrpsee::core::RpcResult;
use jsonrpsee::proc_macros::rpc;
use katana_primitives::{ContractAddress, Felt};
use katana_rpc_types::transaction::InvokeTxResult;
#[cfg_attr(not(feature = "client"), rpc(server, namespace = "cartridge"))]
#[cfg_attr(feature = "client", rpc(client, server, namespace = "cartridge"))]
pub trait CartridgeApi {
    #[method(name = "addExecuteOutsideTransaction")]
    async fn add_execute_outside_transaction(
        &self,
        address: ContractAddress,
        outside_execution: OutsideExecution,
        signature: Vec<Felt>,
    ) -> RpcResult<InvokeTxResult>;
}
