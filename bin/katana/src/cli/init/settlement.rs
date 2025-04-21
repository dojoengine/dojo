use std::sync::Arc;

use katana_primitives::{felt, Felt};
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::JsonRpcClient;
use url::Url;

/// The contract address that handles fact verification.
///
/// Taken from <https://github.com/HerodotusDev/integrity/blob/main/deployed_contracts.md>
const ATLANTIC_FACT_REGISTRY_MAINNET: Felt =
    felt!("0xcc63a1e8e7824642b89fa6baf996b8ed21fa4707be90ef7605570ca8e4f00b");

/// The contract address that handles fact verification.
///
/// This address points to Herodotus' Atlantic Fact Registry contract on Starknet Sepolia as we rely
/// on their services to generates and verifies proofs.
///
/// Taken from <https://github.com/HerodotusDev/integrity/blob/main/deployed_contracts.md>
const ATLANTIC_FACT_REGISTRY_SEPOLIA: Felt =
    felt!("0x4ce7851f00b6c3289674841fd7a1b96b6fd41ed1edc248faccd672c26371b8c");

const CARTRIDGE_SN_MAINNET_PROVIDER: &str = "https://api.cartridge.gg/x/starknet/mainnet";
const CARTRIDGE_SN_SEPOLIA_PROVIDER: &str = "https://api.cartridge.gg/x/starknet/sepolia";

#[derive(Debug, Clone)]
pub struct SettlementChainProvider {
    /// The address of the fact registry contract on the settlement chain.
    fact_registry: Felt,
    /// The JSON-RPC client.
    client: Arc<JsonRpcClient<HttpTransport>>,
    /// The URL for the JSON-RPC client.
    url: Url,
}

impl SettlementChainProvider {
    pub fn sn_mainnet() -> Self {
        let url = Url::parse(CARTRIDGE_SN_MAINNET_PROVIDER).expect("valid url");
        Self::new(url, ATLANTIC_FACT_REGISTRY_MAINNET)
    }

    pub fn sn_sepolia() -> Self {
        let url = Url::parse(CARTRIDGE_SN_SEPOLIA_PROVIDER).expect("valid url");
        Self::new(url, ATLANTIC_FACT_REGISTRY_SEPOLIA)
    }

    pub fn new(url: Url, fact_registry: Felt) -> Self {
        let client = Arc::new(JsonRpcClient::new(HttpTransport::new(url.clone())));
        Self { fact_registry, client, url }
    }

    pub fn set_fact_registry(&mut self, fact_registry: Felt) {
        self.fact_registry = fact_registry;
    }

    /// Returns the address of the fact registry contract.
    pub fn fact_registry(&self) -> Felt {
        self.fact_registry
    }

    /// Returns the URL for the JSON-RPC client.
    pub fn url(&self) -> &Url {
        &self.url
    }
}

mod provider {
    use starknet::core::types::{
        BlockHashAndNumber, BlockId, BroadcastedDeclareTransaction,
        BroadcastedDeployAccountTransaction, BroadcastedInvokeTransaction, BroadcastedTransaction,
        ContractClass, DeclareTransactionResult, DeployAccountTransactionResult, EventFilter,
        EventsPage, FeeEstimate, Felt, FunctionCall, InvokeTransactionResult,
        MaybePendingBlockWithReceipts, MaybePendingBlockWithTxHashes, MaybePendingBlockWithTxs,
        MaybePendingStateUpdate, MsgFromL1, SimulatedTransaction, SimulationFlag,
        SimulationFlagForEstimateFee, SyncStatusType, Transaction, TransactionReceiptWithBlockInfo,
        TransactionStatus, TransactionTrace, TransactionTraceWithHash,
    };
    use starknet::providers::{Provider, ProviderError, ProviderRequestData, ProviderResponseData};

    #[async_trait::async_trait]
    impl Provider for super::SettlementChainProvider {
        async fn spec_version(&self) -> Result<String, ProviderError> {
            self.client.spec_version().await
        }

        async fn get_block_with_tx_hashes<B>(
            &self,
            block_id: B,
        ) -> Result<MaybePendingBlockWithTxHashes, ProviderError>
        where
            B: AsRef<BlockId> + Send + Sync,
        {
            self.client.get_block_with_tx_hashes(block_id).await
        }

        async fn get_block_with_txs<B>(
            &self,
            block_id: B,
        ) -> Result<MaybePendingBlockWithTxs, ProviderError>
        where
            B: AsRef<BlockId> + Send + Sync,
        {
            self.client.get_block_with_txs(block_id).await
        }

        async fn get_block_with_receipts<B>(
            &self,
            block_id: B,
        ) -> Result<MaybePendingBlockWithReceipts, ProviderError>
        where
            B: AsRef<BlockId> + Send + Sync,
        {
            self.client.get_block_with_receipts(block_id).await
        }

        async fn get_state_update<B>(
            &self,
            block_id: B,
        ) -> Result<MaybePendingStateUpdate, ProviderError>
        where
            B: AsRef<BlockId> + Send + Sync,
        {
            self.client.get_state_update(block_id).await
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
            self.client.get_storage_at(contract_address, key, block_id).await
        }

        async fn get_transaction_status<H>(
            &self,
            transaction_hash: H,
        ) -> Result<TransactionStatus, ProviderError>
        where
            H: AsRef<Felt> + Send + Sync,
        {
            self.client.get_transaction_status(transaction_hash).await
        }

        async fn get_transaction_by_hash<H>(
            &self,
            transaction_hash: H,
        ) -> Result<Transaction, ProviderError>
        where
            H: AsRef<Felt> + Send + Sync,
        {
            self.client.get_transaction_by_hash(transaction_hash).await
        }

        async fn get_transaction_by_block_id_and_index<B>(
            &self,
            block_id: B,
            index: u64,
        ) -> Result<Transaction, ProviderError>
        where
            B: AsRef<BlockId> + Send + Sync,
        {
            self.client.get_transaction_by_block_id_and_index(block_id, index).await
        }

        async fn get_transaction_receipt<H>(
            &self,
            transaction_hash: H,
        ) -> Result<TransactionReceiptWithBlockInfo, ProviderError>
        where
            H: AsRef<Felt> + Send + Sync,
        {
            self.client.get_transaction_receipt(transaction_hash).await
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
            self.client.get_class(block_id, class_hash).await
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
            self.client.get_class_hash_at(block_id, contract_address).await
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
            self.client.get_class_at(block_id, contract_address).await
        }

        async fn get_block_transaction_count<B>(&self, block_id: B) -> Result<u64, ProviderError>
        where
            B: AsRef<BlockId> + Send + Sync,
        {
            self.client.get_block_transaction_count(block_id).await
        }

        async fn call<R, B>(&self, request: R, block_id: B) -> Result<Vec<Felt>, ProviderError>
        where
            R: AsRef<FunctionCall> + Send + Sync,
            B: AsRef<BlockId> + Send + Sync,
        {
            self.client.call(request, block_id).await
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
            self.client.estimate_fee(request, simulation_flags, block_id).await
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
            self.client.estimate_message_fee(message, block_id).await
        }

        async fn block_number(&self) -> Result<u64, ProviderError> {
            self.client.block_number().await
        }

        async fn block_hash_and_number(&self) -> Result<BlockHashAndNumber, ProviderError> {
            self.client.block_hash_and_number().await
        }

        async fn chain_id(&self) -> Result<Felt, ProviderError> {
            self.client.chain_id().await
        }

        async fn syncing(&self) -> Result<SyncStatusType, ProviderError> {
            self.client.syncing().await
        }

        async fn get_events(
            &self,
            filter: EventFilter,
            continuation_token: Option<String>,
            chunk_size: u64,
        ) -> Result<EventsPage, ProviderError> {
            self.client.get_events(filter, continuation_token, chunk_size).await
        }

        async fn get_nonce<B, A>(
            &self,
            block_id: B,
            contract_address: A,
        ) -> Result<Felt, ProviderError>
        where
            B: AsRef<BlockId> + Send + Sync,
            A: AsRef<Felt> + Send + Sync,
        {
            self.client.get_nonce(block_id, contract_address).await
        }

        async fn add_invoke_transaction<I>(
            &self,
            invoke_transaction: I,
        ) -> Result<InvokeTransactionResult, ProviderError>
        where
            I: AsRef<BroadcastedInvokeTransaction> + Send + Sync,
        {
            self.client.add_invoke_transaction(invoke_transaction).await
        }

        async fn add_declare_transaction<D>(
            &self,
            declare_transaction: D,
        ) -> Result<DeclareTransactionResult, ProviderError>
        where
            D: AsRef<BroadcastedDeclareTransaction> + Send + Sync,
        {
            self.client.add_declare_transaction(declare_transaction).await
        }

        async fn add_deploy_account_transaction<D>(
            &self,
            deploy_account_transaction: D,
        ) -> Result<DeployAccountTransactionResult, ProviderError>
        where
            D: AsRef<BroadcastedDeployAccountTransaction> + Send + Sync,
        {
            self.client.add_deploy_account_transaction(deploy_account_transaction).await
        }

        async fn trace_transaction<H>(
            &self,
            transaction_hash: H,
        ) -> Result<TransactionTrace, ProviderError>
        where
            H: AsRef<Felt> + Send + Sync,
        {
            self.client.trace_transaction(transaction_hash).await
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
            self.client.simulate_transactions(block_id, transactions, simulation_flags).await
        }

        async fn trace_block_transactions<B>(
            &self,
            block_id: B,
        ) -> Result<Vec<TransactionTraceWithHash>, ProviderError>
        where
            B: AsRef<BlockId> + Send + Sync,
        {
            self.client.trace_block_transactions(block_id).await
        }

        async fn batch_requests<R>(
            &self,
            requests: R,
        ) -> Result<Vec<ProviderResponseData>, ProviderError>
        where
            R: AsRef<[ProviderRequestData]> + Send + Sync,
        {
            self.client.batch_requests(requests).await
        }
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use starknet::core::types::{BlockId, BlockTag};
    use starknet::providers::Provider;

    use super::*;

    #[rstest]
    #[case(SettlementChainProvider::sn_mainnet())]
    #[case(SettlementChainProvider::sn_sepolia())]
    #[tokio::test]
    async fn test_fact_registry_exists(#[case] provider: SettlementChainProvider) {
        let facts_registry = provider.fact_registry();
        let result = provider
            .get_class_hash_at(BlockId::Tag(BlockTag::Pending), facts_registry)
            .await
            .unwrap();

        assert!(result != Felt::ZERO);
    }
}
