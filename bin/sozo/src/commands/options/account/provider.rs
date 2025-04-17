use async_trait::async_trait;
use starknet::core::types::{
    BlockHashAndNumber, BlockId, BroadcastedDeclareTransaction,
    BroadcastedDeployAccountTransaction, BroadcastedInvokeTransaction, BroadcastedTransaction,
    ConfirmedBlockId, ContractClass, ContractStorageKeys, DeclareTransactionResult,
    DeployAccountTransactionResult, EventFilter, EventsPage, FeeEstimate, Felt, FunctionCall,
    Hash256, InvokeTransactionResult, MaybePendingBlockWithReceipts, MaybePendingBlockWithTxHashes,
    MaybePendingBlockWithTxs, MaybePendingStateUpdate, MessageWithStatus, MsgFromL1,
    SimulatedTransaction, SimulationFlag, SimulationFlagForEstimateFee, StorageProof,
    SyncStatusType, Transaction, TransactionReceiptWithBlockInfo, TransactionStatus,
    TransactionTrace, TransactionTraceWithHash,
};
use starknet::providers::{Provider, ProviderError, ProviderRequestData, ProviderResponseData};

#[derive(Debug, Clone)]
pub enum EitherProvider<P, Q>
where
    P: Provider,
    Q: Provider,
{
    Left(P),
    Right(Q),
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<P, Q> Provider for EitherProvider<P, Q>
where
    P: Provider + Sync,
    Q: Provider + Sync,
{
    async fn spec_version(&self) -> Result<String, ProviderError> {
        match self {
            Self::Left(p) => p.spec_version().await,
            Self::Right(q) => q.spec_version().await,
        }
    }

    async fn get_block_with_tx_hashes<B>(
        &self,
        block_id: B,
    ) -> Result<MaybePendingBlockWithTxHashes, ProviderError>
    where
        B: AsRef<BlockId> + Send + Sync,
    {
        match self {
            Self::Left(p) => p.get_block_with_tx_hashes(block_id).await,
            Self::Right(q) => q.get_block_with_tx_hashes(block_id).await,
        }
    }

    async fn get_block_with_txs<B>(
        &self,
        block_id: B,
    ) -> Result<MaybePendingBlockWithTxs, ProviderError>
    where
        B: AsRef<BlockId> + Send + Sync,
    {
        match self {
            Self::Left(p) => p.get_block_with_txs(block_id).await,
            Self::Right(q) => q.get_block_with_txs(block_id).await,
        }
    }

    async fn get_block_with_receipts<B>(
        &self,
        block_id: B,
    ) -> Result<MaybePendingBlockWithReceipts, ProviderError>
    where
        B: AsRef<BlockId> + Send + Sync,
    {
        match self {
            Self::Left(p) => p.get_block_with_receipts(block_id).await,
            Self::Right(q) => q.get_block_with_receipts(block_id).await,
        }
    }

    async fn get_state_update<B>(
        &self,
        block_id: B,
    ) -> Result<MaybePendingStateUpdate, ProviderError>
    where
        B: AsRef<BlockId> + Send + Sync,
    {
        match self {
            Self::Left(p) => p.get_state_update(block_id).await,
            Self::Right(q) => q.get_state_update(block_id).await,
        }
    }

    async fn get_storage_at<A, K, B>(
        &self,
        contract_address: A,
        key: K,
        block_id: B,
    ) -> Result<Felt, ProviderError>
    where
        A: AsRef<Felt> + Send + Sync,
        K: AsRef<Felt> + Send + Sync,
        B: AsRef<BlockId> + Send + Sync,
    {
        match self {
            Self::Left(p) => p.get_storage_at(contract_address, key, block_id).await,
            Self::Right(q) => q.get_storage_at(contract_address, key, block_id).await,
        }
    }

    async fn get_messages_status(
        &self,
        transaction_hash: Hash256,
    ) -> Result<Vec<MessageWithStatus>, ProviderError> {
        match self {
            Self::Left(p) => p.get_messages_status(transaction_hash).await,
            Self::Right(q) => q.get_messages_status(transaction_hash).await,
        }
    }

    async fn get_transaction_status<H>(
        &self,
        transaction_hash: H,
    ) -> Result<TransactionStatus, ProviderError>
    where
        H: AsRef<Felt> + Send + Sync,
    {
        match self {
            Self::Left(p) => p.get_transaction_status(transaction_hash).await,
            Self::Right(q) => q.get_transaction_status(transaction_hash).await,
        }
    }

    async fn get_transaction_by_hash<H>(
        &self,
        transaction_hash: H,
    ) -> Result<Transaction, ProviderError>
    where
        H: AsRef<Felt> + Send + Sync,
    {
        match self {
            Self::Left(p) => p.get_transaction_by_hash(transaction_hash).await,
            Self::Right(q) => q.get_transaction_by_hash(transaction_hash).await,
        }
    }

    async fn get_transaction_by_block_id_and_index<B>(
        &self,
        block_id: B,
        index: u64,
    ) -> Result<Transaction, ProviderError>
    where
        B: AsRef<BlockId> + Send + Sync,
    {
        match self {
            Self::Left(p) => p.get_transaction_by_block_id_and_index(block_id, index).await,
            Self::Right(q) => q.get_transaction_by_block_id_and_index(block_id, index).await,
        }
    }

    async fn get_transaction_receipt<H>(
        &self,
        transaction_hash: H,
    ) -> Result<TransactionReceiptWithBlockInfo, ProviderError>
    where
        H: AsRef<Felt> + Send + Sync,
    {
        match self {
            Self::Left(p) => p.get_transaction_receipt(transaction_hash).await,
            Self::Right(q) => q.get_transaction_receipt(transaction_hash).await,
        }
    }

    async fn get_class<B, H>(
        &self,
        block_id: B,
        class_hash: H,
    ) -> Result<ContractClass, ProviderError>
    where
        B: AsRef<BlockId> + Send + Sync,
        H: AsRef<Felt> + Send + Sync,
    {
        match self {
            Self::Left(p) => p.get_class(block_id, class_hash).await,
            Self::Right(q) => q.get_class(block_id, class_hash).await,
        }
    }

    async fn get_class_hash_at<B, A>(
        &self,
        block_id: B,
        contract_address: A,
    ) -> Result<Felt, ProviderError>
    where
        B: AsRef<BlockId> + Send + Sync,
        A: AsRef<Felt> + Send + Sync,
    {
        match self {
            Self::Left(p) => p.get_class_hash_at(block_id, contract_address).await,
            Self::Right(q) => q.get_class_hash_at(block_id, contract_address).await,
        }
    }

    async fn get_class_at<B, A>(
        &self,
        block_id: B,
        contract_address: A,
    ) -> Result<ContractClass, ProviderError>
    where
        B: AsRef<BlockId> + Send + Sync,
        A: AsRef<Felt> + Send + Sync,
    {
        match self {
            Self::Left(p) => p.get_class_at(block_id, contract_address).await,
            Self::Right(q) => q.get_class_at(block_id, contract_address).await,
        }
    }

    async fn get_block_transaction_count<B>(&self, block_id: B) -> Result<u64, ProviderError>
    where
        B: AsRef<BlockId> + Send + Sync,
    {
        match self {
            Self::Left(p) => p.get_block_transaction_count(block_id).await,
            Self::Right(q) => q.get_block_transaction_count(block_id).await,
        }
    }

    async fn call<R, B>(&self, request: R, block_id: B) -> Result<Vec<Felt>, ProviderError>
    where
        R: AsRef<FunctionCall> + Send + Sync,
        B: AsRef<BlockId> + Send + Sync,
    {
        match self {
            Self::Left(p) => p.call(request, block_id).await,
            Self::Right(q) => q.call(request, block_id).await,
        }
    }

    async fn estimate_fee<R, S, B>(
        &self,
        request: R,
        simulation_flags: S,
        block_id: B,
    ) -> Result<Vec<FeeEstimate>, ProviderError>
    where
        R: AsRef<[BroadcastedTransaction]> + Send + Sync,
        S: AsRef<[SimulationFlagForEstimateFee]> + Send + Sync,
        B: AsRef<BlockId> + Send + Sync,
    {
        match self {
            Self::Left(p) => p.estimate_fee(request, simulation_flags, block_id).await,
            Self::Right(q) => q.estimate_fee(request, simulation_flags, block_id).await,
        }
    }

    async fn estimate_message_fee<M, B>(
        &self,
        message: M,
        block_id: B,
    ) -> Result<FeeEstimate, ProviderError>
    where
        M: AsRef<MsgFromL1> + Send + Sync,
        B: AsRef<BlockId> + Send + Sync,
    {
        match self {
            Self::Left(p) => p.estimate_message_fee(message, block_id).await,
            Self::Right(q) => q.estimate_message_fee(message, block_id).await,
        }
    }

    async fn block_number(&self) -> Result<u64, ProviderError> {
        match self {
            Self::Left(p) => p.block_number().await,
            Self::Right(q) => q.block_number().await,
        }
    }

    async fn block_hash_and_number(&self) -> Result<BlockHashAndNumber, ProviderError> {
        match self {
            Self::Left(p) => p.block_hash_and_number().await,
            Self::Right(q) => q.block_hash_and_number().await,
        }
    }

    async fn chain_id(&self) -> Result<Felt, ProviderError> {
        match self {
            Self::Left(p) => p.chain_id().await,
            Self::Right(q) => q.chain_id().await,
        }
    }

    async fn syncing(&self) -> Result<SyncStatusType, ProviderError> {
        match self {
            Self::Left(p) => p.syncing().await,
            Self::Right(q) => q.syncing().await,
        }
    }

    async fn get_events(
        &self,
        filter: EventFilter,
        continuation_token: Option<String>,
        chunk_size: u64,
    ) -> Result<EventsPage, ProviderError> {
        match self {
            Self::Left(p) => p.get_events(filter, continuation_token, chunk_size).await,
            Self::Right(q) => q.get_events(filter, continuation_token, chunk_size).await,
        }
    }

    async fn get_nonce<B, A>(&self, block_id: B, contract_address: A) -> Result<Felt, ProviderError>
    where
        B: AsRef<BlockId> + Send + Sync,
        A: AsRef<Felt> + Send + Sync,
    {
        match self {
            Self::Left(p) => p.get_nonce(block_id, contract_address).await,
            Self::Right(q) => q.get_nonce(block_id, contract_address).await,
        }
    }

    async fn get_storage_proof<B, H, A, K>(
        &self,
        block_id: B,
        class_hashes: H,
        contract_addresses: A,
        contracts_storage_keys: K,
    ) -> Result<StorageProof, ProviderError>
    where
        B: AsRef<ConfirmedBlockId> + Send + Sync,
        H: AsRef<[Felt]> + Send + Sync,
        A: AsRef<[Felt]> + Send + Sync,
        K: AsRef<[ContractStorageKeys]> + Send + Sync,
    {
        match self {
            Self::Left(p) => {
                p.get_storage_proof(
                    block_id,
                    class_hashes,
                    contract_addresses,
                    contracts_storage_keys,
                )
                .await
            }
            Self::Right(q) => {
                q.get_storage_proof(
                    block_id,
                    class_hashes,
                    contract_addresses,
                    contracts_storage_keys,
                )
                .await
            }
        }
    }

    async fn add_invoke_transaction<I>(
        &self,
        invoke_transaction: I,
    ) -> Result<InvokeTransactionResult, ProviderError>
    where
        I: AsRef<BroadcastedInvokeTransaction> + Send + Sync,
    {
        match self {
            Self::Left(p) => p.add_invoke_transaction(invoke_transaction).await,
            Self::Right(q) => q.add_invoke_transaction(invoke_transaction).await,
        }
    }

    async fn add_declare_transaction<D>(
        &self,
        declare_transaction: D,
    ) -> Result<DeclareTransactionResult, ProviderError>
    where
        D: AsRef<BroadcastedDeclareTransaction> + Send + Sync,
    {
        match self {
            Self::Left(p) => p.add_declare_transaction(declare_transaction).await,
            Self::Right(q) => q.add_declare_transaction(declare_transaction).await,
        }
    }

    async fn add_deploy_account_transaction<D>(
        &self,
        deploy_account_transaction: D,
    ) -> Result<DeployAccountTransactionResult, ProviderError>
    where
        D: AsRef<BroadcastedDeployAccountTransaction> + Send + Sync,
    {
        match self {
            Self::Left(p) => p.add_deploy_account_transaction(deploy_account_transaction).await,
            Self::Right(q) => q.add_deploy_account_transaction(deploy_account_transaction).await,
        }
    }

    async fn trace_transaction<H>(
        &self,
        transaction_hash: H,
    ) -> Result<TransactionTrace, ProviderError>
    where
        H: AsRef<Felt> + Send + Sync,
    {
        match self {
            Self::Left(p) => p.trace_transaction(transaction_hash).await,
            Self::Right(q) => q.trace_transaction(transaction_hash).await,
        }
    }

    async fn simulate_transactions<B, T, S>(
        &self,
        block_id: B,
        transactions: T,
        simulation_flags: S,
    ) -> Result<Vec<SimulatedTransaction>, ProviderError>
    where
        B: AsRef<BlockId> + Send + Sync,
        T: AsRef<[BroadcastedTransaction]> + Send + Sync,
        S: AsRef<[SimulationFlag]> + Send + Sync,
    {
        match self {
            Self::Left(p) => {
                p.simulate_transactions(block_id, transactions, simulation_flags).await
            }
            Self::Right(q) => {
                q.simulate_transactions(block_id, transactions, simulation_flags).await
            }
        }
    }

    async fn trace_block_transactions<B>(
        &self,
        block_id: B,
    ) -> Result<Vec<TransactionTraceWithHash>, ProviderError>
    where
        B: AsRef<BlockId> + Send + Sync,
    {
        match self {
            Self::Left(p) => p.trace_block_transactions(block_id).await,
            Self::Right(q) => q.trace_block_transactions(block_id).await,
        }
    }

    async fn batch_requests<R>(
        &self,
        requests: R,
    ) -> Result<Vec<ProviderResponseData>, ProviderError>
    where
        R: AsRef<[ProviderRequestData]> + Send + Sync,
    {
        match self {
            Self::Left(p) => p.batch_requests(requests).await,
            Self::Right(q) => q.batch_requests(requests).await,
        }
    }
}
