use katana_db::error::DatabaseError;
use katana_primitives::block::BlockNumber;
use katana_primitives::class::ClassHash;
use katana_primitives::contract::{ContractAddress, StorageKey};
use katana_primitives::transaction::TxNumber;

use crate::providers::fork::backend::ForkedBackendError;

/// Possible errors returned by the storage provider.
#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    /// Error for anything related to parsing data.
    #[error("Parsing error: {0}")]
    ParsingError(String),

    #[error("Missing latest block hash")]
    MissingLatestBlockHash,

    #[error("Missing latest block number")]
    MissingLatestBlockNumber,

    /// Error when the block hash is not found when it should be.
    #[error("Missing block hash for block number {0}")]
    MissingBlockHash(BlockNumber),

    /// Error when the block header is not found when it should be.
    #[error("Missing block header for block number {0}")]
    MissingBlockHeader(BlockNumber),

    /// Error when the block body is not found but the block exists.
    #[error("Missing block transactions for block number {0}")]
    MissingBlockTxs(BlockNumber),

    /// Error when the block body indices are not found but the block exists.
    #[error("Missing block body indices for block number {0}")]
    MissingBlockBodyIndices(BlockNumber),

    /// Error when the block status is not found but the block exists.
    #[error("Missing block status for block number {0}")]
    MissingBlockStatus(BlockNumber),

    /// Error when a full transaction data is not found but its hash/number exists.
    #[error("Missing transaction for tx number {0}")]
    MissingTx(TxNumber),

    /// Error when a transaction block number is not found but the transaction exists.
    #[error("Missing transaction block number for tx number {0}")]
    MissingTxBlock(TxNumber),

    /// Error when a transaction hash is not found but the transaction exists.
    #[error("Missing transaction hash for tx number {0}")]
    MissingTxHash(TxNumber),

    /// Error when a transaction receipt is not found but the transaction exists.
    #[error("Missing transaction receipt for tx number {0}")]
    MissingTxReceipt(TxNumber),

    /// Error when a compiled class hash is not found but the class hash exists.
    #[error("Missing compiled class hash for class hash {0:#x}")]
    MissingCompiledClassHash(ClassHash),

    /// Error when a contract class change entry is not found but the block number of when the
    /// change happen exists in the class change list.
    #[error("Missing contract class change entry")]
    MissingContractClassChangeEntry {
        /// The block number of when the change happen.
        block: BlockNumber,
        /// The updated contract address.
        contract_address: ContractAddress,
    },

    /// Error when a contract nonce change entry is not found but the block number of when the
    /// change happen exists in the nonce change list.
    #[error(
        "Missing contract nonce change entry for contract {contract_address} at block {block}"
    )]
    MissingContractNonceChangeEntry {
        /// The block number of when the change happen.
        block: BlockNumber,
        /// The updated contract address.
        contract_address: ContractAddress,
    },

    /// Error when a storage change entry is not found but the block number of when the change
    /// happen exists in the storage change list.
    #[error(
        "Missing storage change entry for contract {contract_address} at block {block} for key \
         {storage_key:#x}"
    )]
    MissingStorageChangeEntry {
        /// The block number of when the change happen.
        block: BlockNumber,
        /// The updated contract address.
        contract_address: ContractAddress,
        /// The updated storage key.
        storage_key: StorageKey,
    },

    /// Error returned by the database implementation.
    #[error(transparent)]
    Database(#[from] DatabaseError),

    /// Error returned by a [ForkedBackend](crate::providers::fork::backend::ForkedBackend) used by
    /// [ForkedProvider](crate::providers::fork::ForkedProvider).
    #[cfg(feature = "fork")]
    #[error(transparent)]
    ForkedBackend(#[from] ForkedBackendError),

    /// Any error that is not covered by the other variants.
    #[error("soemthing went wrong: {0}")]
    Other(String),
}
