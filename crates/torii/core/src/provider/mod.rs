pub mod provider;
pub mod transport;

use std::{any::Any, error::Error, fmt::Display};

use async_trait::async_trait;
use katana_rpc_types::transaction::{TransactionsPage, TransactionsPageCursor};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_with::serde_as;
use starknet::core::{
    serde::unsigned_field_element::UfeHex,
    types::{
        requests::*, BlockHashAndNumber, BlockId, BroadcastedDeclareTransaction,
        BroadcastedDeployAccountTransaction, BroadcastedInvokeTransaction, BroadcastedTransaction,
        ContractClass, ContractErrorData, DeclareTransactionResult, DeployAccountTransactionResult,
        EventFilter, EventFilterWithPage, EventsPage, FeeEstimate, FieldElement, FunctionCall,
        InvokeTransactionResult, MaybePendingBlockWithTxHashes, MaybePendingBlockWithTxs,
        MaybePendingStateUpdate, MaybePendingTransactionReceipt, MsgFromL1,
        NoTraceAvailableErrorData, ResultPageRequest, SimulatedTransaction, SimulationFlag,
        SimulationFlagForEstimateFee, StarknetError, SyncStatusType, Transaction,
        TransactionExecutionErrorData, TransactionStatus, TransactionTrace,
        TransactionTraceWithHash,
    },
};

use provider::{ProviderImplError, Provider, ProviderError};

use self::transport::JsonRpcTransport;

#[derive(Debug)]
pub struct JsonRpcClient<T> {
    transport: T,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum JsonRpcMethod {
    #[serde(rename = "starknet_specVersion")]
    SpecVersion,
    #[serde(rename = "starknet_getBlockWithTxHashes")]
    GetBlockWithTxHashes,
    #[serde(rename = "starknet_getBlockWithTxs")]
    GetBlockWithTxs,
    #[serde(rename = "starknet_getStateUpdate")]
    GetStateUpdate,
    #[serde(rename = "starknet_getStorageAt")]
    GetStorageAt,
    #[serde(rename = "starknet_getTransactionStatus")]
    GetTransactionStatus,
    #[serde(rename = "starknet_getTransactionByHash")]
    GetTransactionByHash,
    #[serde(rename = "starknet_getTransactionByBlockIdAndIndex")]
    GetTransactionByBlockIdAndIndex,
    #[serde(rename = "starknet_getTransactionReceipt")]
    GetTransactionReceipt,
    #[serde(rename = "starknet_getClass")]
    GetClass,
    #[serde(rename = "starknet_getClassHashAt")]
    GetClassHashAt,
    #[serde(rename = "starknet_getClassAt")]
    GetClassAt,
    #[serde(rename = "starknet_getBlockTransactionCount")]
    GetBlockTransactionCount,
    #[serde(rename = "starknet_call")]
    Call,
    #[serde(rename = "starknet_estimateFee")]
    EstimateFee,
    #[serde(rename = "starknet_estimateMessageFee")]
    EstimateMessageFee,
    #[serde(rename = "starknet_blockNumber")]
    BlockNumber,
    #[serde(rename = "starknet_blockHashAndNumber")]
    BlockHashAndNumber,
    #[serde(rename = "starknet_chainId")]
    ChainId,
    #[serde(rename = "starknet_syncing")]
    Syncing,
    #[serde(rename = "starknet_getEvents")]
    GetEvents,
    #[serde(rename = "starknet_getNonce")]
    GetNonce,
    #[serde(rename = "starknet_addInvokeTransaction")]
    AddInvokeTransaction,
    #[serde(rename = "starknet_addDeclareTransaction")]
    AddDeclareTransaction,
    #[serde(rename = "starknet_addDeployAccountTransaction")]
    AddDeployAccountTransaction,
    #[serde(rename = "starknet_traceTransaction")]
    TraceTransaction,
    #[serde(rename = "starknet_simulateTransactions")]
    SimulateTransactions,
    #[serde(rename = "starknet_traceBlockTransactions")]
    TraceBlockTransactions,
    #[serde(rename = "torii_getTransactions")]
    GetTransactions,
}

#[derive(Debug, Clone)]
pub struct JsonRpcRequest {
    pub id: u64,
    pub data: JsonRpcRequestData,
}

#[derive(Debug, Clone)]
pub enum JsonRpcRequestData {
    SpecVersion(SpecVersionRequest),
    GetBlockWithTxHashes(GetBlockWithTxHashesRequest),
    GetBlockWithTxs(GetBlockWithTxsRequest),
    GetStateUpdate(GetStateUpdateRequest),
    GetStorageAt(GetStorageAtRequest),
    GetTransactionStatus(GetTransactionStatusRequest),
    GetTransactionByHash(GetTransactionByHashRequest),
    GetTransactionByBlockIdAndIndex(GetTransactionByBlockIdAndIndexRequest),
    GetTransactionReceipt(GetTransactionReceiptRequest),
    GetClass(GetClassRequest),
    GetClassHashAt(GetClassHashAtRequest),
    GetClassAt(GetClassAtRequest),
    GetBlockTransactionCount(GetBlockTransactionCountRequest),
    Call(CallRequest),
    EstimateFee(EstimateFeeRequest),
    EstimateMessageFee(EstimateMessageFeeRequest),
    BlockNumber(BlockNumberRequest),
    BlockHashAndNumber(BlockHashAndNumberRequest),
    ChainId(ChainIdRequest),
    Syncing(SyncingRequest),
    GetEvents(GetEventsRequest),
    GetNonce(GetNonceRequest),
    AddInvokeTransaction(AddInvokeTransactionRequest),
    AddDeclareTransaction(AddDeclareTransactionRequest),
    AddDeployAccountTransaction(AddDeployAccountTransactionRequest),
    TraceTransaction(TraceTransactionRequest),
    SimulateTransactions(SimulateTransactionsRequest),
    TraceBlockTransactions(TraceBlockTransactionsRequest),
    GetTransactions(TransactionsPageCursor),
}

#[derive(Debug, thiserror::Error)]
pub enum JsonRpcClientError<T> {
    #[error(transparent)]
    JsonError(serde_json::Error),
    #[error(transparent)]
    TransportError(T),
    #[error(transparent)]
    JsonRpcError(JsonRpcError),
}

#[derive(Debug, Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum JsonRpcResponse<T> {
    Success { id: u64, result: T },
    Error { id: u64, error: JsonRpcError },
}

/// Failures trying to parse a [JsonRpcError] into [StarknetError].
#[derive(Debug, thiserror::Error)]
pub enum JsonRpcErrorConversionError {
    #[error("unknown error code")]
    UnknownCode,
    #[error("missing data field")]
    MissingData,
    #[error("unable to parse the data field")]
    DataParsingFailure,
}

#[serde_as]
#[derive(Serialize, Deserialize)]
struct Felt(#[serde_as(as = "UfeHex")] pub FieldElement);

#[serde_as]
#[derive(Serialize, Deserialize)]
struct FeltArray(#[serde_as(as = "Vec<UfeHex>")] pub Vec<FieldElement>);

impl<T> JsonRpcClient<T> {
    pub fn new(transport: T) -> Self {
        Self { transport }
    }
}

impl<T> JsonRpcClient<T>
where
    T: 'static + JsonRpcTransport + Send + Sync,
{
    async fn send_request<P, R>(&self, method: JsonRpcMethod, params: P) -> Result<R, ProviderError>
    where
        P: Serialize + Send + Sync,
        R: DeserializeOwned,
    {
        match self
            .transport
            .send_request(method, params)
            .await
            .map_err(JsonRpcClientError::TransportError)?
        {
            JsonRpcResponse::Success { result, .. } => Ok(result),
            JsonRpcResponse::Error { error, .. } => {
                Err(match TryInto::<StarknetError>::try_into(&error) {
                    Ok(error) => ProviderError::StarknetError(error),
                    Err(_) => JsonRpcClientError::<T::Error>::JsonRpcError(error).into(),
                })
            }
        }
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<T> Provider for JsonRpcClient<T>
where
    T: 'static + JsonRpcTransport + Sync + Send,
{
    /// Returns the version of the Starknet JSON-RPC specification being used
    async fn spec_version(&self) -> Result<String, ProviderError> {
        self.send_request(JsonRpcMethod::SpecVersion, SpecVersionRequest)
            .await
    }

    /// Get block information with transaction hashes given the block id
    async fn get_block_with_tx_hashes<B>(
        &self,
        block_id: B,
    ) -> Result<MaybePendingBlockWithTxHashes, ProviderError>
    where
        B: AsRef<BlockId> + Send + Sync,
    {
        self.send_request(
            JsonRpcMethod::GetBlockWithTxHashes,
            GetBlockWithTxHashesRequestRef {
                block_id: block_id.as_ref(),
            },
        )
        .await
    }

    /// Get block information with full transactions given the block id
    async fn get_block_with_txs<B>(
        &self,
        block_id: B,
    ) -> Result<MaybePendingBlockWithTxs, ProviderError>
    where
        B: AsRef<BlockId> + Send + Sync,
    {
        self.send_request(
            JsonRpcMethod::GetBlockWithTxs,
            GetBlockWithTxsRequestRef {
                block_id: block_id.as_ref(),
            },
        )
        .await
    }

    /// Get the information about the result of executing the requested block
    async fn get_state_update<B>(
        &self,
        block_id: B,
    ) -> Result<MaybePendingStateUpdate, ProviderError>
    where
        B: AsRef<BlockId> + Send + Sync,
    {
        self.send_request(
            JsonRpcMethod::GetStateUpdate,
            GetStateUpdateRequestRef {
                block_id: block_id.as_ref(),
            },
        )
        .await
    }

    /// Get the value of the storage at the given address and key
    async fn get_storage_at<A, K, B>(
        &self,
        contract_address: A,
        key: K,
        block_id: B,
    ) -> Result<FieldElement, ProviderError>
    where
        A: AsRef<FieldElement> + Send + Sync,
        K: AsRef<FieldElement> + Send + Sync,
        B: AsRef<BlockId> + Send + Sync,
    {
        Ok(self
            .send_request::<_, Felt>(
                JsonRpcMethod::GetStorageAt,
                GetStorageAtRequestRef {
                    contract_address: contract_address.as_ref(),
                    key: key.as_ref(),
                    block_id: block_id.as_ref(),
                },
            )
            .await?
            .0)
    }

    /// Gets the transaction status (possibly reflecting that the tx is still in
    /// the mempool, or dropped from it)
    async fn get_transaction_status<H>(
        &self,
        transaction_hash: H,
    ) -> Result<TransactionStatus, ProviderError>
    where
        H: AsRef<FieldElement> + Send + Sync,
    {
        self.send_request(
            JsonRpcMethod::GetTransactionStatus,
            GetTransactionStatusRequestRef {
                transaction_hash: transaction_hash.as_ref(),
            },
        )
        .await
    }

    /// Get the details and status of a submitted transaction
    async fn get_transaction_by_hash<H>(
        &self,
        transaction_hash: H,
    ) -> Result<Transaction, ProviderError>
    where
        H: AsRef<FieldElement> + Send + Sync,
    {
        self.send_request(
            JsonRpcMethod::GetTransactionByHash,
            GetTransactionByHashRequestRef {
                transaction_hash: transaction_hash.as_ref(),
            },
        )
        .await
    }

    /// Get the details of a transaction by a given block id and index
    async fn get_transaction_by_block_id_and_index<B>(
        &self,
        block_id: B,
        index: u64,
    ) -> Result<Transaction, ProviderError>
    where
        B: AsRef<BlockId> + Send + Sync,
    {
        self.send_request(
            JsonRpcMethod::GetTransactionByBlockIdAndIndex,
            GetTransactionByBlockIdAndIndexRequestRef {
                block_id: block_id.as_ref(),
                index: &index,
            },
        )
        .await
    }

    /// Get the details of a transaction by a given block number and index
    async fn get_transaction_receipt<H>(
        &self,
        transaction_hash: H,
    ) -> Result<MaybePendingTransactionReceipt, ProviderError>
    where
        H: AsRef<FieldElement> + Send + Sync,
    {
        self.send_request(
            JsonRpcMethod::GetTransactionReceipt,
            GetTransactionReceiptRequestRef {
                transaction_hash: transaction_hash.as_ref(),
            },
        )
        .await
    }

    /// Get the contract class definition in the given block associated with the given hash
    async fn get_class<B, H>(
        &self,
        block_id: B,
        class_hash: H,
    ) -> Result<ContractClass, ProviderError>
    where
        B: AsRef<BlockId> + Send + Sync,
        H: AsRef<FieldElement> + Send + Sync,
    {
        self.send_request(
            JsonRpcMethod::GetClass,
            GetClassRequestRef {
                block_id: block_id.as_ref(),
                class_hash: class_hash.as_ref(),
            },
        )
        .await
    }

    /// Get the contract class hash in the given block for the contract deployed at the given address
    async fn get_class_hash_at<B, A>(
        &self,
        block_id: B,
        contract_address: A,
    ) -> Result<FieldElement, ProviderError>
    where
        B: AsRef<BlockId> + Send + Sync,
        A: AsRef<FieldElement> + Send + Sync,
    {
        Ok(self
            .send_request::<_, Felt>(
                JsonRpcMethod::GetClassHashAt,
                GetClassHashAtRequestRef {
                    block_id: block_id.as_ref(),
                    contract_address: contract_address.as_ref(),
                },
            )
            .await?
            .0)
    }

    /// Get the contract class definition in the given block at the given address
    async fn get_class_at<B, A>(
        &self,
        block_id: B,
        contract_address: A,
    ) -> Result<ContractClass, ProviderError>
    where
        B: AsRef<BlockId> + Send + Sync,
        A: AsRef<FieldElement> + Send + Sync,
    {
        self.send_request(
            JsonRpcMethod::GetClassAt,
            GetClassAtRequestRef {
                block_id: block_id.as_ref(),
                contract_address: contract_address.as_ref(),
            },
        )
        .await
    }

    /// Get the number of transactions in a block given a block id
    async fn get_block_transaction_count<B>(&self, block_id: B) -> Result<u64, ProviderError>
    where
        B: AsRef<BlockId> + Send + Sync,
    {
        self.send_request(
            JsonRpcMethod::GetBlockTransactionCount,
            GetBlockTransactionCountRequestRef {
                block_id: block_id.as_ref(),
            },
        )
        .await
    }

    /// Call a starknet function without creating a Starknet transaction
    async fn call<R, B>(&self, request: R, block_id: B) -> Result<Vec<FieldElement>, ProviderError>
    where
        R: AsRef<FunctionCall> + Send + Sync,
        B: AsRef<BlockId> + Send + Sync,
    {
        Ok(self
            .send_request::<_, FeltArray>(
                JsonRpcMethod::Call,
                CallRequestRef {
                    request: request.as_ref(),
                    block_id: block_id.as_ref(),
                },
            )
            .await?
            .0)
    }

    /// Estimate the fee for a given Starknet transaction
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
        self.send_request(
            JsonRpcMethod::EstimateFee,
            EstimateFeeRequestRef {
                request: request.as_ref(),
                simulation_flags: simulation_flags.as_ref(),
                block_id: block_id.as_ref(),
            },
        )
        .await
    }

    /// Estimate the L2 fee of a message sent on L1
    async fn estimate_message_fee<M, B>(
        &self,
        message: M,
        block_id: B,
    ) -> Result<FeeEstimate, ProviderError>
    where
        M: AsRef<MsgFromL1> + Send + Sync,
        B: AsRef<BlockId> + Send + Sync,
    {
        self.send_request(
            JsonRpcMethod::EstimateMessageFee,
            EstimateMessageFeeRequestRef {
                message: message.as_ref(),
                block_id: block_id.as_ref(),
            },
        )
        .await
    }

    /// Get the most recent accepted block number
    async fn block_number(&self) -> Result<u64, ProviderError> {
        self.send_request(JsonRpcMethod::BlockNumber, BlockNumberRequest)
            .await
    }

    /// Get the most recent accepted block hash and number
    async fn block_hash_and_number(&self) -> Result<BlockHashAndNumber, ProviderError> {
        self.send_request(JsonRpcMethod::BlockHashAndNumber, BlockHashAndNumberRequest)
            .await
    }

    /// Return the currently configured Starknet chain id
    async fn chain_id(&self) -> Result<FieldElement, ProviderError> {
        Ok(self
            .send_request::<_, Felt>(JsonRpcMethod::ChainId, ChainIdRequest)
            .await?
            .0)
    }

    /// Returns an object about the sync status, or false if the node is not synching
    async fn syncing(&self) -> Result<SyncStatusType, ProviderError> {
        self.send_request(JsonRpcMethod::Syncing, SyncingRequest)
            .await
    }

    /// Returns all events matching the given filter
    async fn get_events(
        &self,
        filter: EventFilter,
        continuation_token: Option<String>,
        chunk_size: u64,
    ) -> Result<EventsPage, ProviderError> {
        self.send_request(
            JsonRpcMethod::GetEvents,
            GetEventsRequestRef {
                filter: &EventFilterWithPage {
                    event_filter: filter,
                    result_page_request: ResultPageRequest {
                        continuation_token,
                        chunk_size,
                    },
                },
            },
        )
        .await
    }

    /// Get the nonce associated with the given address in the given block
    async fn get_nonce<B, A>(
        &self,
        block_id: B,
        contract_address: A,
    ) -> Result<FieldElement, ProviderError>
    where
        B: AsRef<BlockId> + Send + Sync,
        A: AsRef<FieldElement> + Send + Sync,
    {
        Ok(self
            .send_request::<_, Felt>(
                JsonRpcMethod::GetNonce,
                GetNonceRequestRef {
                    block_id: block_id.as_ref(),
                    contract_address: contract_address.as_ref(),
                },
            )
            .await?
            .0)
    }

    /// Submit a new transaction to be added to the chain
    async fn add_invoke_transaction<I>(
        &self,
        invoke_transaction: I,
    ) -> Result<InvokeTransactionResult, ProviderError>
    where
        I: AsRef<BroadcastedInvokeTransaction> + Send + Sync,
    {
        self.send_request(
            JsonRpcMethod::AddInvokeTransaction,
            AddInvokeTransactionRequestRef {
                invoke_transaction: invoke_transaction.as_ref(),
            },
        )
        .await
    }

    /// Submit a new transaction to be added to the chain
    async fn add_declare_transaction<D>(
        &self,
        declare_transaction: D,
    ) -> Result<DeclareTransactionResult, ProviderError>
    where
        D: AsRef<BroadcastedDeclareTransaction> + Send + Sync,
    {
        self.send_request(
            JsonRpcMethod::AddDeclareTransaction,
            AddDeclareTransactionRequestRef {
                declare_transaction: declare_transaction.as_ref(),
            },
        )
        .await
    }

    /// Submit a new deploy account transaction
    async fn add_deploy_account_transaction<D>(
        &self,
        deploy_account_transaction: D,
    ) -> Result<DeployAccountTransactionResult, ProviderError>
    where
        D: AsRef<BroadcastedDeployAccountTransaction> + Send + Sync,
    {
        self.send_request(
            JsonRpcMethod::AddDeployAccountTransaction,
            AddDeployAccountTransactionRequestRef {
                deploy_account_transaction: deploy_account_transaction.as_ref(),
            },
        )
        .await
    }

    /// For a given executed transaction, return the trace of its execution, including internal
    /// calls
    async fn trace_transaction<H>(
        &self,
        transaction_hash: H,
    ) -> Result<TransactionTrace, ProviderError>
    where
        H: AsRef<FieldElement> + Send + Sync,
    {
        self.send_request(
            JsonRpcMethod::TraceTransaction,
            TraceTransactionRequestRef {
                transaction_hash: transaction_hash.as_ref(),
            },
        )
        .await
    }

    /// Simulate a given sequence of transactions on the requested state, and generate the execution
    /// traces. Note that some of the transactions may revert, in which case no error is thrown, but
    /// revert details can be seen on the returned trace object. . Note that some of the
    /// transactions may revert, this will be reflected by the revert_error property in the trace.
    /// Other types of failures (e.g. unexpected error or failure in the validation phase) will
    /// result in TRANSACTION_EXECUTION_ERROR.
    async fn simulate_transactions<B, TX, S>(
        &self,
        block_id: B,
        transactions: TX,
        simulation_flags: S,
    ) -> Result<Vec<SimulatedTransaction>, ProviderError>
    where
        B: AsRef<BlockId> + Send + Sync,
        TX: AsRef<[BroadcastedTransaction]> + Send + Sync,
        S: AsRef<[SimulationFlag]> + Send + Sync,
    {
        self.send_request(
            JsonRpcMethod::SimulateTransactions,
            SimulateTransactionsRequestRef {
                block_id: block_id.as_ref(),
                transactions: transactions.as_ref(),
                simulation_flags: simulation_flags.as_ref(),
            },
        )
        .await
    }

    /// Retrieve traces for all transactions in the given block.
    async fn trace_block_transactions<B>(
        &self,
        block_id: B,
    ) -> Result<Vec<TransactionTraceWithHash>, ProviderError>
    where
        B: AsRef<BlockId> + Send + Sync,
    {
        self.send_request(
            JsonRpcMethod::TraceBlockTransactions,
            TraceBlockTransactionsRequestRef {
                block_id: block_id.as_ref(),
            },
        )
        .await
    }

    async fn get_transactions(&self, cursor: TransactionsPageCursor) -> Result<TransactionsPage, ProviderError> {
        self.send_request(JsonRpcMethod::GetTransactions, cursor).await
    }
}

impl<'de> Deserialize<'de> for JsonRpcRequest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct RawRequest {
            id: u64,
            method: JsonRpcMethod,
            params: serde_json::Value,
        }

        let error_mapper =
            |err| serde::de::Error::custom(format!("unable to decode params: {}", err));

        let raw_request = RawRequest::deserialize(deserializer)?;
        let request_data = match raw_request.method {
            JsonRpcMethod::SpecVersion => JsonRpcRequestData::SpecVersion(
                serde_json::from_value::<SpecVersionRequest>(raw_request.params)
                    .map_err(error_mapper)?,
            ),
            JsonRpcMethod::GetBlockWithTxHashes => JsonRpcRequestData::GetBlockWithTxHashes(
                serde_json::from_value::<GetBlockWithTxHashesRequest>(raw_request.params)
                    .map_err(error_mapper)?,
            ),
            JsonRpcMethod::GetBlockWithTxs => JsonRpcRequestData::GetBlockWithTxs(
                serde_json::from_value::<GetBlockWithTxsRequest>(raw_request.params)
                    .map_err(error_mapper)?,
            ),
            JsonRpcMethod::GetStateUpdate => JsonRpcRequestData::GetStateUpdate(
                serde_json::from_value::<GetStateUpdateRequest>(raw_request.params)
                    .map_err(error_mapper)?,
            ),
            JsonRpcMethod::GetStorageAt => JsonRpcRequestData::GetStorageAt(
                serde_json::from_value::<GetStorageAtRequest>(raw_request.params)
                    .map_err(error_mapper)?,
            ),
            JsonRpcMethod::GetTransactionStatus => JsonRpcRequestData::GetTransactionStatus(
                serde_json::from_value::<GetTransactionStatusRequest>(raw_request.params)
                    .map_err(error_mapper)?,
            ),
            JsonRpcMethod::GetTransactionByHash => JsonRpcRequestData::GetTransactionByHash(
                serde_json::from_value::<GetTransactionByHashRequest>(raw_request.params)
                    .map_err(error_mapper)?,
            ),
            JsonRpcMethod::GetTransactionByBlockIdAndIndex => {
                JsonRpcRequestData::GetTransactionByBlockIdAndIndex(
                    serde_json::from_value::<GetTransactionByBlockIdAndIndexRequest>(
                        raw_request.params,
                    )
                    .map_err(error_mapper)?,
                )
            }
            JsonRpcMethod::GetTransactionReceipt => JsonRpcRequestData::GetTransactionReceipt(
                serde_json::from_value::<GetTransactionReceiptRequest>(raw_request.params)
                    .map_err(error_mapper)?,
            ),
            JsonRpcMethod::GetClass => JsonRpcRequestData::GetClass(
                serde_json::from_value::<GetClassRequest>(raw_request.params)
                    .map_err(error_mapper)?,
            ),
            JsonRpcMethod::GetClassHashAt => JsonRpcRequestData::GetClassHashAt(
                serde_json::from_value::<GetClassHashAtRequest>(raw_request.params)
                    .map_err(error_mapper)?,
            ),
            JsonRpcMethod::GetClassAt => JsonRpcRequestData::GetClassAt(
                serde_json::from_value::<GetClassAtRequest>(raw_request.params)
                    .map_err(error_mapper)?,
            ),
            JsonRpcMethod::GetBlockTransactionCount => {
                JsonRpcRequestData::GetBlockTransactionCount(
                    serde_json::from_value::<GetBlockTransactionCountRequest>(raw_request.params)
                        .map_err(error_mapper)?,
                )
            }
            JsonRpcMethod::Call => JsonRpcRequestData::Call(
                serde_json::from_value::<CallRequest>(raw_request.params).map_err(error_mapper)?,
            ),
            JsonRpcMethod::EstimateFee => JsonRpcRequestData::EstimateFee(
                serde_json::from_value::<EstimateFeeRequest>(raw_request.params)
                    .map_err(error_mapper)?,
            ),
            JsonRpcMethod::EstimateMessageFee => JsonRpcRequestData::EstimateMessageFee(
                serde_json::from_value::<EstimateMessageFeeRequest>(raw_request.params)
                    .map_err(error_mapper)?,
            ),
            JsonRpcMethod::BlockNumber => JsonRpcRequestData::BlockNumber(
                serde_json::from_value::<BlockNumberRequest>(raw_request.params)
                    .map_err(error_mapper)?,
            ),
            JsonRpcMethod::BlockHashAndNumber => JsonRpcRequestData::BlockHashAndNumber(
                serde_json::from_value::<BlockHashAndNumberRequest>(raw_request.params)
                    .map_err(error_mapper)?,
            ),
            JsonRpcMethod::ChainId => JsonRpcRequestData::ChainId(
                serde_json::from_value::<ChainIdRequest>(raw_request.params)
                    .map_err(error_mapper)?,
            ),
            JsonRpcMethod::Syncing => JsonRpcRequestData::Syncing(
                serde_json::from_value::<SyncingRequest>(raw_request.params)
                    .map_err(error_mapper)?,
            ),
            JsonRpcMethod::GetEvents => JsonRpcRequestData::GetEvents(
                serde_json::from_value::<GetEventsRequest>(raw_request.params)
                    .map_err(error_mapper)?,
            ),
            JsonRpcMethod::GetNonce => JsonRpcRequestData::GetNonce(
                serde_json::from_value::<GetNonceRequest>(raw_request.params)
                    .map_err(error_mapper)?,
            ),
            JsonRpcMethod::AddInvokeTransaction => JsonRpcRequestData::AddInvokeTransaction(
                serde_json::from_value::<AddInvokeTransactionRequest>(raw_request.params)
                    .map_err(error_mapper)?,
            ),
            JsonRpcMethod::AddDeclareTransaction => JsonRpcRequestData::AddDeclareTransaction(
                serde_json::from_value::<AddDeclareTransactionRequest>(raw_request.params)
                    .map_err(error_mapper)?,
            ),
            JsonRpcMethod::AddDeployAccountTransaction => {
                JsonRpcRequestData::AddDeployAccountTransaction(
                    serde_json::from_value::<AddDeployAccountTransactionRequest>(
                        raw_request.params,
                    )
                    .map_err(error_mapper)?,
                )
            }
            JsonRpcMethod::TraceTransaction => JsonRpcRequestData::TraceTransaction(
                serde_json::from_value::<TraceTransactionRequest>(raw_request.params)
                    .map_err(error_mapper)?,
            ),
            JsonRpcMethod::SimulateTransactions => JsonRpcRequestData::SimulateTransactions(
                serde_json::from_value::<SimulateTransactionsRequest>(raw_request.params)
                    .map_err(error_mapper)?,
            ),
            JsonRpcMethod::TraceBlockTransactions => JsonRpcRequestData::TraceBlockTransactions(
                serde_json::from_value::<TraceBlockTransactionsRequest>(raw_request.params)
                    .map_err(error_mapper)?,
            ),
            JsonRpcMethod::GetTransactions => JsonRpcRequestData::GetTransactions(
                serde_json::from_value::<TransactionsPageCursor>(raw_request.params)
                    .map_err(error_mapper)?,
            ),
        };

        Ok(Self {
            id: raw_request.id,
            data: request_data,
        })
    }
}

impl<T> ProviderImplError for JsonRpcClientError<T>
where
    T: 'static + Error + Send + Sync,
{
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl<T> From<JsonRpcClientError<T>> for ProviderError
where
    T: 'static + Error + Send + Sync,
{
    fn from(value: JsonRpcClientError<T>) -> Self {
        Self::Other(Box::new(value))
    }
}

impl<T> From<serde_json::Error> for JsonRpcClientError<T> {
    fn from(value: serde_json::Error) -> Self {
        Self::JsonError(value)
    }
}

impl TryFrom<&JsonRpcError> for StarknetError {
    type Error = JsonRpcErrorConversionError;

    fn try_from(value: &JsonRpcError) -> Result<Self, Self::Error> {
        match value.code {
            1 => Ok(StarknetError::FailedToReceiveTransaction),
            20 => Ok(StarknetError::ContractNotFound),
            24 => Ok(StarknetError::BlockNotFound),
            27 => Ok(StarknetError::InvalidTransactionIndex),
            28 => Ok(StarknetError::ClassHashNotFound),
            29 => Ok(StarknetError::TransactionHashNotFound),
            31 => Ok(StarknetError::PageSizeTooBig),
            32 => Ok(StarknetError::NoBlocks),
            33 => Ok(StarknetError::InvalidContinuationToken),
            34 => Ok(StarknetError::TooManyKeysInFilter),
            40 => {
                let data = ContractErrorData::deserialize(
                    value
                        .data
                        .as_ref()
                        .ok_or(JsonRpcErrorConversionError::MissingData)?,
                )
                .map_err(|_| JsonRpcErrorConversionError::DataParsingFailure)?;
                Ok(StarknetError::ContractError(data))
            }
            41 => {
                let data = TransactionExecutionErrorData::deserialize(
                    value
                        .data
                        .as_ref()
                        .ok_or(JsonRpcErrorConversionError::MissingData)?,
                )
                .map_err(|_| JsonRpcErrorConversionError::DataParsingFailure)?;
                Ok(StarknetError::TransactionExecutionError(data))
            }
            51 => Ok(StarknetError::ClassAlreadyDeclared),
            52 => Ok(StarknetError::InvalidTransactionNonce),
            53 => Ok(StarknetError::InsufficientMaxFee),
            54 => Ok(StarknetError::InsufficientAccountBalance),
            55 => {
                let data = String::deserialize(
                    value
                        .data
                        .as_ref()
                        .ok_or(JsonRpcErrorConversionError::MissingData)?,
                )
                .map_err(|_| JsonRpcErrorConversionError::DataParsingFailure)?;
                Ok(StarknetError::ValidationFailure(data))
            }
            56 => Ok(StarknetError::CompilationFailed),
            57 => Ok(StarknetError::ContractClassSizeIsTooLarge),
            58 => Ok(StarknetError::NonAccount),
            59 => Ok(StarknetError::DuplicateTx),
            60 => Ok(StarknetError::CompiledClassHashMismatch),
            61 => Ok(StarknetError::UnsupportedTxVersion),
            62 => Ok(StarknetError::UnsupportedContractClassVersion),
            63 => {
                let data = String::deserialize(
                    value
                        .data
                        .as_ref()
                        .ok_or(JsonRpcErrorConversionError::MissingData)?,
                )
                .map_err(|_| JsonRpcErrorConversionError::DataParsingFailure)?;
                Ok(StarknetError::UnexpectedError(data))
            }
            10 => {
                let data = NoTraceAvailableErrorData::deserialize(
                    value
                        .data
                        .as_ref()
                        .ok_or(JsonRpcErrorConversionError::MissingData)?,
                )
                .map_err(|_| JsonRpcErrorConversionError::DataParsingFailure)?;
                Ok(StarknetError::NoTraceAvailable(data))
            }
            _ => Err(JsonRpcErrorConversionError::UnknownCode),
        }
    }
}

impl Error for JsonRpcError {}

impl Display for JsonRpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.data {
            Some(data) => {
                write!(
                    f,
                    "JSON-RPC error: code={}, message=\"{}\", data={}",
                    self.code,
                    self.message,
                    serde_json::to_string(data).map_err(|_| std::fmt::Error)?
                )
            }
            None => {
                write!(
                    f,
                    "JSON-RPC error: code={}, message=\"{}\"",
                    self.code, self.message
                )
            }
        }
    }
}