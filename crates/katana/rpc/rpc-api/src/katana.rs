use jsonrpsee::core::RpcResult;
use jsonrpsee::proc_macros::rpc;
use katana_primitives::FieldElement;
use katana_rpc_types::account::Account;

#[cfg_attr(not(feature = "client"), rpc(server, namespace = "katana"))]
#[cfg_attr(feature = "client", rpc(client, server, namespace = "katana"))]
pub trait KatanaApi {
    #[method(name = "generateBlock")]
    async fn generate_block(&self) -> RpcResult<()>;

    #[method(name = "nextBlockTimestamp")]
    async fn next_block_timestamp(&self) -> RpcResult<u64>;

    #[method(name = "setNextBlockTimestamp")]
    async fn set_next_block_timestamp(&self, timestamp: u64) -> RpcResult<()>;

    #[method(name = "increaseNextBlockTimestamp")]
    async fn increase_next_block_timestamp(&self, timestamp: u64) -> RpcResult<()>;

    #[method(name = "predeployedAccounts")]
    async fn predeployed_accounts(&self) -> RpcResult<Vec<Account>>;

    #[method(name = "setStorageAt")]
    async fn set_storage_at(
        &self,
        contract_address: FieldElement,
        key: FieldElement,
        value: FieldElement,
    ) -> RpcResult<()>;
}
