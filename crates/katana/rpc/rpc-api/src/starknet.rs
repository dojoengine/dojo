use jsonrpsee::core::RpcResult;
use jsonrpsee::proc_macros::rpc;
use katana_primitives::block::{BlockIdOrTag, BlockNumber};
use katana_primitives::transaction::TxHash;
use katana_primitives::FieldElement;
use katana_rpc_types::block::{
    BlockHashAndNumber, BlockTxCount, MaybePendingBlockWithTxHashes, MaybePendingBlockWithTxs,
};
use katana_rpc_types::event::{EventFilterWithPage, EventsPage};
use katana_rpc_types::message::MsgFromL1;
use katana_rpc_types::receipt::MaybePendingTxReceipt;
use katana_rpc_types::state_update::StateUpdate;
use katana_rpc_types::transaction::{
    BroadcastedDeclareTx, BroadcastedDeployAccountTx, BroadcastedInvokeTx, BroadcastedTx,
    DeclareTxResult, DeployAccountTxResult, InvokeTxResult, Tx,
};
use katana_rpc_types::{
    ContractClass, FeeEstimate, FeltAsHex, FunctionCall, SimulationFlags, SyncingStatus,
};
use starknet::core::types::TransactionStatus;

/// The currently supported version of the Starknet JSON-RPC specification.
pub const RPC_SPEC_VERSION: &str = "0.6.0";

/// Starknet JSON-RPC APIs: <https://github.com/starkware-libs/starknet-specs>
#[cfg_attr(not(feature = "client"), rpc(server, namespace = "starknet"))]
#[cfg_attr(feature = "client", rpc(client, server, namespace = "starknet"))]
pub trait StarknetApi {
    /// Returns the version of the Starknet JSON-RPC specification being used.
    #[method(name = "specVersion")]
    async fn spec_version(&self) -> RpcResult<String> {
        Ok(RPC_SPEC_VERSION.into())
    }

    /// Get block information with transaction hashes given the block id.
    #[method(name = "getBlockWithTxHashes")]
    async fn block_with_tx_hashes(
        &self,
        block_id: BlockIdOrTag,
    ) -> RpcResult<MaybePendingBlockWithTxHashes>;

    /// Get block information with full transactions given the block id.
    #[method(name = "getBlockWithTxs")]
    async fn block_with_txs(&self, block_id: BlockIdOrTag) -> RpcResult<MaybePendingBlockWithTxs>;

    /// Get the information about the result of executing the requested block.
    #[method(name = "getStateUpdate")]
    async fn state_update(&self, block_id: BlockIdOrTag) -> RpcResult<StateUpdate>;

    /// Get the value of the storage at the given address and key
    #[method(name = "getStorageAt")]
    async fn storage_at(
        &self,
        contract_address: FieldElement,
        key: FieldElement,
        block_id: BlockIdOrTag,
    ) -> RpcResult<FeltAsHex>;

    /// Gets the transaction status (possibly reflecting that the tx is still in the mempool, or
    /// dropped from it).
    #[method(name = "getTransactionStatus")]
    async fn transaction_status(&self, transaction_hash: TxHash) -> RpcResult<TransactionStatus>;

    /// Get the details and status of a submitted transaction.
    #[method(name = "getTransactionByHash")]
    async fn transaction_by_hash(&self, transaction_hash: TxHash) -> RpcResult<Tx>;

    /// Get the details of a transaction by a given block id and index.
    #[method(name = "getTransactionByBlockIdAndIndex")]
    async fn transaction_by_block_id_and_index(
        &self,
        block_id: BlockIdOrTag,
        index: u64,
    ) -> RpcResult<Tx>;

    /// Get the transaction receipt by the transaction hash.
    #[method(name = "getTransactionReceipt")]
    async fn transaction_receipt(
        &self,
        transaction_hash: TxHash,
    ) -> RpcResult<MaybePendingTxReceipt>;

    /// Get the contract class definition in the given block associated with the given hash.
    #[method(name = "getClass")]
    async fn class(
        &self,
        block_id: BlockIdOrTag,
        class_hash: FieldElement,
    ) -> RpcResult<ContractClass>;

    /// Get the contract class hash in the given block for the contract deployed at the given
    /// address.
    #[method(name = "getClassHashAt")]
    async fn class_hash_at(
        &self,
        block_id: BlockIdOrTag,
        contract_address: FieldElement,
    ) -> RpcResult<FeltAsHex>;

    /// Get the contract class definition in the given block at the given address.
    #[method(name = "getClassAt")]
    async fn class_at(
        &self,
        block_id: BlockIdOrTag,
        contract_address: FieldElement,
    ) -> RpcResult<ContractClass>;

    /// Get the number of transactions in a block given a block id.
    #[method(name = "getBlockTransactionCount")]
    async fn block_transaction_count(&self, block_id: BlockIdOrTag) -> RpcResult<BlockTxCount>;

    /// Call a starknet function without creating a StarkNet transaction.
    #[method(name = "call")]
    async fn call(
        &self,
        request: FunctionCall,
        block_id: BlockIdOrTag,
    ) -> RpcResult<Vec<FeltAsHex>>;

    /// Estimate the fee for of StarkNet transactions.
    #[method(name = "estimateFee")]
    async fn estimate_fee(
        &self,
        request: Vec<BroadcastedTx>,
        simulation_flags: Vec<SimulationFlags>,
        block_id: BlockIdOrTag,
    ) -> RpcResult<Vec<FeeEstimate>>;

    /// Estimate the L2 fee of a message sent on L1.
    #[method(name = "estimateMessageFee")]
    async fn estimate_message_fee(
        &self,
        message: MsgFromL1,
        block_id: BlockIdOrTag,
    ) -> RpcResult<FeeEstimate>;

    /// Get the most recent accepted block number.
    #[method(name = "blockNumber")]
    async fn block_number(&self) -> RpcResult<BlockNumber>;

    /// Get the most recent accepted block hash and number.
    #[method(name = "blockHashAndNumber")]
    async fn block_hash_and_number(&self) -> RpcResult<BlockHashAndNumber>;

    /// Return the currently configured StarkNet chain id.
    #[method(name = "chainId")]
    async fn chain_id(&self) -> RpcResult<FeltAsHex>;

    /// Returns an object about the sync status, or false if the node is not synching.
    #[method(name = "syncing")]
    async fn syncing(&self) -> RpcResult<SyncingStatus> {
        Ok(SyncingStatus::NotSyncing)
    }

    /// Returns all event objects matching the conditions in the provided filter.
    #[method(name = "getEvents")]
    async fn events(&self, filter: EventFilterWithPage) -> RpcResult<EventsPage>;

    /// Get the nonce associated with the given address in the given block.
    #[method(name = "getNonce")]
    async fn nonce(
        &self,
        block_id: BlockIdOrTag,
        contract_address: FieldElement,
    ) -> RpcResult<FeltAsHex>;

    /// Submit a new transaction to be added to the chain.
    #[method(name = "addInvokeTransaction")]
    async fn add_invoke_transaction(
        &self,
        invoke_transaction: BroadcastedInvokeTx,
    ) -> RpcResult<InvokeTxResult>;

    /// Submit a new class declaration transaction.
    #[method(name = "addDeclareTransaction")]
    async fn add_declare_transaction(
        &self,
        declare_transaction: BroadcastedDeclareTx,
    ) -> RpcResult<DeclareTxResult>;

    /// Submit a new deploy account transaction.
    #[method(name = "addDeployAccountTransaction")]
    async fn add_deploy_account_transaction(
        &self,
        deploy_account_transaction: BroadcastedDeployAccountTx,
    ) -> RpcResult<DeployAccountTxResult>;
}
