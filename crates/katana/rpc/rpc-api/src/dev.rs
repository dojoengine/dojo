use jsonrpsee::core::RpcResult;
use jsonrpsee::proc_macros::rpc;
use katana_primitives::FieldElement;

#[cfg_attr(not(feature = "client"), rpc(server, namespace = "dev"))]
#[cfg_attr(feature = "client", rpc(client, server, namespace = "dev"))]
pub trait DevApi {
    #[method(name = "generateBlock")]
    async fn generate_block(&self) -> RpcResult<()>;

    #[method(name = "nextBlockTimestamp")]
    async fn next_block_timestamp(&self) -> RpcResult<()>;

    #[method(name = "setNextBlockTimestamp")]
    async fn set_next_block_timestamp(&self, timestamp: u64) -> RpcResult<()>;

    #[method(name = "increaseNextBlockTimestamp")]
    async fn increase_next_block_timestamp(&self, timestamp: u64) -> RpcResult<()>;

    #[method(name = "setStorageAt")]
    async fn set_storage_at(
        &self,
        contract_address: FieldElement,
        key: FieldElement,
        value: FieldElement,
    ) -> RpcResult<()>;
}
