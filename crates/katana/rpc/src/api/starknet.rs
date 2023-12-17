use jsonrpsee::core::Error;
use jsonrpsee::proc_macros::rpc;
use jsonrpsee::types::error::{CallError, ErrorObject};
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
use katana_rpc_types::{ContractClass, FeeEstimate, FeltAsHex, FunctionCall};
use starknet::core::types::{ContractErrorData, TransactionStatus};

#[derive(thiserror::Error, Clone, Debug)]
#[repr(i32)]
pub enum StarknetApiError {
    #[error("Failed to write transaction")]
    FailedToReceiveTxn,
    #[error("Contract not found")]
    ContractNotFound,
    #[error("Invalid message selector")]
    InvalidMessageSelector,
    #[error("Invalid call data")]
    InvalidCallData,
    #[error("Block not found")]
    BlockNotFound,
    #[error("Transaction hash not found")]
    TxnHashNotFound,
    #[error("Invalid transaction index in a block")]
    InvalidTxnIndex,
    #[error("Class hash not found")]
    ClassHashNotFound,
    #[error("Requested page size is too big")]
    PageSizeTooBig,
    #[error("There are no blocks")]
    NoBlocks,
    #[error("The supplied continuation token is invalid or unknown")]
    InvalidContinuationToken,
    #[error("Contract error")]
    ContractError { revert_error: String },
    #[error("Invalid contract class")]
    InvalidContractClass,
    #[error("Class already declared")]
    ClassAlreadyDeclared,
    #[error("Invalid transaction nonce")]
    InvalidTransactionNonce,
    #[error("Max fee is smaller than the minimal transaction cost (validation plus fee transfer)")]
    InsufficientMaxFee,
    #[error("Account balance is smaller than the transaction's max_fee")]
    InsufficientAccountBalance,
    #[error("Account validation failed")]
    ValidationFailure,
    #[error("Compilation failed")]
    CompilationFailed,
    #[error("Contract class size is too large")]
    ContractClassSizeIsTooLarge,
    #[error("Sender address in not an account contract")]
    NonAccount,
    #[error("A transaction with the same hash already exists in the mempool")]
    DuplicateTransaction,
    #[error("The compiled class hash did not match the one supplied in the transaction")]
    CompiledClassHashMismatch,
    #[error("The transaction version is not supported")]
    UnsupportedTransactionVersion,
    #[error("The contract class version is not supported")]
    UnsupportedContractClassVersion,
    #[error("An unexpected error occured")]
    UnexpectedError,
    #[error("Too many storage keys requested")]
    ProofLimitExceeded,
    #[error("Too many keys provided in a filter")]
    TooManyKeysInFilter,
    #[error("Failed to fetch pending transactions")]
    FailedToFetchPendingTransactions,
}

impl From<StarknetApiError> for Error {
    fn from(err: StarknetApiError) -> Self {
        let message = err.to_string();

        let (code, data) = match err {
            StarknetApiError::FailedToReceiveTxn => (1, None),
            StarknetApiError::ContractNotFound => (20, None),
            StarknetApiError::InvalidMessageSelector => (21, None),
            StarknetApiError::InvalidCallData => (22, None),
            StarknetApiError::BlockNotFound => (24, None),
            StarknetApiError::TxnHashNotFound => (29, None),
            StarknetApiError::InvalidTxnIndex => (27, None),
            StarknetApiError::ClassHashNotFound => (28, None),
            StarknetApiError::PageSizeTooBig => (31, None),
            StarknetApiError::NoBlocks => (32, None),
            StarknetApiError::InvalidContinuationToken => (33, None),
            StarknetApiError::ContractError { revert_error } => {
                (40, Some(ContractErrorData { revert_error }))
            }
            StarknetApiError::InvalidContractClass => (50, None),
            StarknetApiError::ClassAlreadyDeclared => (51, None),
            StarknetApiError::InvalidTransactionNonce => (52, None),
            StarknetApiError::InsufficientMaxFee => (53, None),
            StarknetApiError::InsufficientAccountBalance => (54, None),
            StarknetApiError::ValidationFailure => (55, None),
            StarknetApiError::CompilationFailed => (56, None),
            StarknetApiError::ContractClassSizeIsTooLarge => (57, None),
            StarknetApiError::NonAccount => (58, None),
            StarknetApiError::DuplicateTransaction => (59, None),
            StarknetApiError::CompiledClassHashMismatch => (60, None),
            StarknetApiError::UnsupportedTransactionVersion => (61, None),
            StarknetApiError::UnsupportedContractClassVersion => (62, None),
            StarknetApiError::UnexpectedError => (63, None),
            StarknetApiError::ProofLimitExceeded => (10000, None),
            StarknetApiError::TooManyKeysInFilter => (34, None),
            StarknetApiError::FailedToFetchPendingTransactions => (38, None),
        };

        Error::Call(CallError::Custom(ErrorObject::owned(code, message, data)))
    }
}

#[rpc(server, namespace = "starknet")]
pub trait StarknetApi {
    // Read API

    #[method(name = "specVersion")]
    async fn spec_version(&self) -> Result<String, Error> {
        Ok("0.5.1".into())
    }

    #[method(name = "chainId")]
    async fn chain_id(&self) -> Result<FeltAsHex, Error>;

    #[method(name = "getNonce")]
    async fn nonce(
        &self,
        block_id: BlockIdOrTag,
        contract_address: FieldElement,
    ) -> Result<FeltAsHex, Error>;

    #[method(name = "blockNumber")]
    async fn block_number(&self) -> Result<BlockNumber, Error>;

    #[method(name = "getTransactionByHash")]
    async fn transaction_by_hash(&self, transaction_hash: TxHash) -> Result<Tx, Error>;

    #[method(name = "getBlockTransactionCount")]
    async fn block_transaction_count(&self, block_id: BlockIdOrTag) -> Result<BlockTxCount, Error>;

    #[method(name = "getClassAt")]
    async fn class_at(
        &self,
        block_id: BlockIdOrTag,
        contract_address: FieldElement,
    ) -> Result<ContractClass, Error>;

    #[method(name = "blockHashAndNumber")]
    async fn block_hash_and_number(&self) -> Result<BlockHashAndNumber, Error>;

    #[method(name = "getBlockWithTxHashes")]
    async fn block_with_tx_hashes(
        &self,
        block_id: BlockIdOrTag,
    ) -> Result<MaybePendingBlockWithTxHashes, Error>;

    #[method(name = "getTransactionByBlockIdOrTagAndIndex")]
    async fn transaction_by_block_id_and_index(
        &self,
        block_id: BlockIdOrTag,
        index: u64,
    ) -> Result<Tx, Error>;

    #[method(name = "getBlockWithTxs")]
    async fn block_with_txs(
        &self,
        block_id: BlockIdOrTag,
    ) -> Result<MaybePendingBlockWithTxs, Error>;

    #[method(name = "getStateUpdate")]
    async fn state_update(&self, block_id: BlockIdOrTag) -> Result<StateUpdate, Error>;

    #[method(name = "getTransactionReceipt")]
    async fn transaction_receipt(
        &self,
        transaction_hash: TxHash,
    ) -> Result<MaybePendingTxReceipt, Error>;

    #[method(name = "getTransactionStatus")]
    async fn transaction_status(
        &self,
        transaction_hash: TxHash,
    ) -> Result<TransactionStatus, Error>;

    #[method(name = "getClassHashAt")]
    async fn class_hash_at(
        &self,
        block_id: BlockIdOrTag,
        contract_address: FieldElement,
    ) -> Result<FeltAsHex, Error>;

    #[method(name = "getClass")]
    async fn class(
        &self,
        block_id: BlockIdOrTag,
        class_hash: FieldElement,
    ) -> Result<ContractClass, Error>;

    #[method(name = "getEvents")]
    async fn events(&self, filter: EventFilterWithPage) -> Result<EventsPage, Error>;

    #[method(name = "estimateFee")]
    async fn estimate_fee(
        &self,
        request: Vec<BroadcastedTx>,
        block_id: BlockIdOrTag,
    ) -> Result<Vec<FeeEstimate>, Error>;

    #[method(name = "estimateMessageFee")]
    async fn estimate_message_fee(
        &self,
        message: MsgFromL1,
        block_id: BlockIdOrTag,
    ) -> Result<FeeEstimate, Error>;

    #[method(name = "call")]
    async fn call(
        &self,
        request: FunctionCall,
        block_id: BlockIdOrTag,
    ) -> Result<Vec<FeltAsHex>, Error>;

    #[method(name = "getStorageAt")]
    async fn storage_at(
        &self,
        contract_address: FieldElement,
        key: FieldElement,
        block_id: BlockIdOrTag,
    ) -> Result<FeltAsHex, Error>;

    // Write API

    #[method(name = "addDeployAccountTransaction")]
    async fn add_deploy_account_transaction(
        &self,
        deploy_account_transaction: BroadcastedDeployAccountTx,
    ) -> Result<DeployAccountTxResult, Error>;

    #[method(name = "addDeclareTransaction")]
    async fn add_declare_transaction(
        &self,
        declare_transaction: BroadcastedDeclareTx,
    ) -> Result<DeclareTxResult, Error>;

    #[method(name = "addInvokeTransaction")]
    async fn add_invoke_transaction(
        &self,
        invoke_transaction: BroadcastedInvokeTx,
    ) -> Result<InvokeTxResult, Error>;
}
