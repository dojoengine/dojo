use jsonrpsee::{
    core::Error,
    proc_macros::rpc,
    types::error::{CallError, ErrorObject},
};

use starknet::{
    core::types::FieldElement,
    providers::jsonrpc::models::{
        BlockHashAndNumber, BlockId, BroadcastedInvokeTransaction, ContractClass,
        DeclareTransactionResult, DeployTransactionResult, EventFilter, EventsPage, FeeEstimate,
        FunctionCall, InvokeTransactionResult, MaybePendingBlockWithTxHashes,
        MaybePendingBlockWithTxs, MaybePendingTransactionReceipt, StateUpdate, Transaction,
    },
};

#[derive(thiserror::Error, Clone, Copy, Debug)]
pub enum KatanaApiError {
    #[error("Failed to write transaction")]
    FailedToReceiveTxn = 1,
    #[error("Contract not found")]
    ContractNotFound = 20,
    #[error("Invalid message selector")]
    InvalidMessageSelector = 21,
    #[error("Invalid call data")]
    InvalidCallData = 22,
    #[error("Block not found")]
    BlockNotFound = 24,
    #[error("Transaction hash not found")]
    TxnHashNotFound = 25,
    #[error("Invalid transaction index in a block")]
    InvalidTxnIndex = 27,
    #[error("Class hash not found")]
    ClassHashNotFound = 28,
    #[error("Requested page size is too big")]
    PageSizeTooBig = 31,
    #[error("There are no blocks")]
    NoBlocks = 32,
    #[error("The supplied continuation token is invalid or unknown")]
    InvalidContinuationToken = 33,
    #[error("Contract error")]
    ContractError = 40,
    #[error("Invalid contract class")]
    InvalidContractClass = 50,
    #[error("Too many storage keys requested")]
    ProofLimitExceeded = 10000,
    #[error("Too many keys provided in a filter")]
    TooManyKeysInFilter = 34,
    #[error("Internal server error")]
    InternalServerError = 500,
    #[error("Failed to fetch pending transactions")]
    FailedToFetchPendingTransactions = 38,
}

impl From<KatanaApiError> for Error {
    fn from(err: KatanaApiError) -> Self {
        Error::Call(CallError::Custom(ErrorObject::owned(
            err as i32,
            err.to_string(),
            None::<()>,
        )))
    }
}

#[rpc(server, client, namespace = "starknet")]
pub trait KatanaApi {
    #[method(name = "chainId")]
    async fn chain_id(&self) -> Result<String, Error> {
        unimplemented!("chain_id");
    }

    #[method(name = "getNonce")]
    async fn get_nonce(&self, _contract_address: String) -> Result<String, Error> {
        unimplemented!("get_nonce");
    }

    #[method(name = "blockNumber")]
    async fn block_number(&self) -> Result<u64, Error> {
        unimplemented!("block_number");
    }

    #[method(name = "getTransactionByHash")]
    async fn get_transaction_by_hash(&self, _tx_hash: &str) -> Result<Transaction, Error> {
        unimplemented!("get_transaction_by_hash");
    }

    #[method(name = "getBlockTransactionCount")]
    async fn get_block_transaction_count(&self, _block_id: BlockId) -> Result<u64, Error> {
        unimplemented!("get_block_transaction_count");
    }

    #[method(name = "getClassAt")]
    async fn get_class_at(
        &self,
        _block_id: BlockId,
        _contract_address: String,
    ) -> Result<ContractClass, Error> {
        unimplemented!("get_class_at");
    }

    #[method(name = "blockHashAndNumber")]
    async fn block_hash_and_number(&self) -> Result<BlockHashAndNumber, Error> {
        unimplemented!("block_hash_and_number");
    }

    #[method(name = "getBlockWithTxHashes")]
    async fn get_block_with_tx_hashes(
        &self,
        _block_id: BlockId,
    ) -> Result<MaybePendingBlockWithTxHashes, Error> {
        unimplemented!("get_block_with_tx_hashes");
    }

    #[method(name = "getTransactionByBlockIdAndIndex")]
    async fn get_transaction_by_block_id_and_index(
        &self,
        _block_id: BlockId,
        _index: &str,
    ) -> Result<Transaction, Error> {
        unimplemented!("get_transaction_by_block_id_and_index");
    }

    #[method(name = "addInvokeTransaction")]
    async fn add_invoke_transaction(
        &self,
        _invoke_transaction: BroadcastedInvokeTransaction,
    ) -> Result<InvokeTransactionResult, Error> {
        unimplemented!("add_invoke_transaction");
    }

    #[method(name = "getBlockWithTxs")]
    async fn get_block_with_txs(
        &self,
        _block_id: BlockId,
    ) -> Result<MaybePendingBlockWithTxs, Error> {
        unimplemented!("get_block_with_txs");
    }

    #[method(name = "getStateUpdate")]
    async fn get_state_update(&self, _block_id: BlockId) -> Result<StateUpdate, Error> {
        unimplemented!("get_state_update");
    }

    #[method(name = "getTransactionReceipt")]
    async fn get_transaction_receipt(
        &self,
        _tx_hash: String,
    ) -> Result<MaybePendingTransactionReceipt, Error> {
        unimplemented!("get_transaction_receipt");
    }

    #[method(name = "getClassHashAt")]
    async fn get_class_hash_at(
        &self,
        _block_id: BlockId,
        _contract_address: String,
    ) -> Result<FieldElement, Error> {
        unimplemented!("get_class_hash_at");
    }

    #[method(name = "getClass")]
    async fn get_class(
        &self,
        _block_id: BlockId,
        _class_hash: String,
    ) -> Result<ContractClass, Error> {
        unimplemented!("get_class");
    }

    #[method(name = "addDeployAccountTransaction")]
    async fn add_deploy_account_transaction(
        &self,
        _contract_class: String,
        _version: String,
        _contract_address_salt: String,
        _constructor_calldata: Vec<String>,
    ) -> Result<DeployTransactionResult, Error> {
        unimplemented!("add_deploy_account_transaction");
    }

    #[method(name = "getEvents")]
    async fn get_events(
        &self,
        _filter: EventFilter,
        _continuation_token: Option<String>,
        _chunk_size: u64,
    ) -> Result<EventsPage, Error> {
        unimplemented!("get_events");
    }

    #[method(name = "addDeclareTransaction")]
    async fn add_declare_transaction(
        &self,
        _version: String,
        _max_fee: String,
        _signature: Vec<String>,
        _nonce: String,
        _contract_class: String,
        _sender_address: String,
    ) -> Result<DeclareTransactionResult, Error> {
        unimplemented!("add_declare_transaction");
    }

    #[method(name = "pendingTransactions")]
    async fn pending_transactions(&self) -> Result<Vec<Transaction>, Error> {
        unimplemented!("pending_transactions");
    }

    #[method(name = "estimateFee")]
    async fn estimate_fee(
        &self,
        _block_id: BlockId,
        _broadcasted_transaction: String,
    ) -> Result<FeeEstimate, Error> {
        unimplemented!("estimate_fee");
    }

    #[method(name = "call")]
    async fn call(
        &self,
        _request: FunctionCall,
        _block_number: u64,
    ) -> Result<Vec<FieldElement>, Error> {
        unimplemented!("call");
    }

    #[method(name = "getStorageAt")]
    async fn get_storage_at(
        &self,
        _contract_address: String,
        _key: String,
    ) -> Result<FieldElement, Error> {
        unimplemented!("get_storage_at");
    }
}
