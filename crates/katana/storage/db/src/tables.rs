use katana_primitives::block::{BlockHash, BlockNumber, FinalityStatus, Header};
use katana_primitives::class::{ClassHash, CompiledClass, CompiledClassHash, FlattenedSierraClass};
use katana_primitives::contract::{ContractAddress, GenericContractInfo, Nonce, StorageKey};
use katana_primitives::receipt::Receipt;
use katana_primitives::trace::TxExecInfo;
use katana_primitives::transaction::{Tx, TxHash, TxNumber};

use crate::codecs::{Compress, Decode, Decompress, Encode};
use crate::models::block::StoredBlockBodyIndices;
use crate::models::contract::{ContractClassChange, ContractInfoChangeList, ContractNonceChange};
use crate::models::list::BlockList;
use crate::models::storage::{ContractStorageEntry, ContractStorageKey, MessagingCheckpointId, StorageEntry};

pub trait Key: Encode + Decode + Clone + std::fmt::Debug {}
pub trait Value: Compress + Decompress + std::fmt::Debug {}

impl<T> Key for T where T: Encode + Decode + Clone + std::fmt::Debug {}
impl<T> Value for T where T: Compress + Decompress + std::fmt::Debug {}

/// An asbtraction for a table.
pub trait Table {
    /// The name of the table.
    const NAME: &'static str;
    /// The key type of the table.
    type Key: Key;
    /// The value type of the table.
    type Value: Value;
}

/// DupSort allows for keys to be repeated in the database.
///
/// Upstream docs: <https://libmdbx.dqdkfa.ru/usage.html#autotoc_md48>
pub trait DupSort: Table {
    /// Upstream docs: <https://libmdbx.dqdkfa.ru/usage.html#autotoc_md48>
    type SubKey: Key;
}

/// Enum for the types of tables present in libmdbx.
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum TableType {
    /// key value table
    Table,
    /// Duplicate key value table
    DupSort,
}

pub const NUM_TABLES: usize = 27;

/// Macro to declare `libmdbx` tables.
#[macro_export]
macro_rules! define_tables_enum {
    { [$(($table:ident, $type:expr)),*] } => {
        #[derive(Debug, PartialEq, Copy, Clone)]
        /// Default tables that should be present inside database.
        pub enum Tables {
            $(
                $table,
            )*
        }

        impl Tables {
            /// Array of all tables in database
            pub const ALL: [Tables; NUM_TABLES] = [$(Tables::$table,)*];

            /// The name of the given table in database
            pub const fn name(&self) -> &str {
                match self {
                    $(Tables::$table => {
                        $table::NAME
                    },)*
                }
            }

            /// The type of the given table in database
            pub const fn table_type(&self) -> TableType {
                match self {
                    $(Tables::$table => {
                        $type
                    },)*
                }
            }
        }

        impl std::fmt::Display for Tables {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.name())
            }
        }

        impl std::str::FromStr for Tables {
            type Err = String;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match s {
                    $($table::NAME => {
                        return Ok(Tables::$table)
                    },)*
                    _ => {
                        return Err(format!("unknown table `{s}`"))
                    }
                }
            }
        }
    };
}

/// Macro to declare key value table.
#[macro_export]
macro_rules! tables {
    { $( $(#[$docs:meta])+ $table_name:ident: ($key:ty $(,$key_type2:ty)?) => $value:ty ),* } => {
       $(
            $(#[$docs])+
            ///
            #[doc = concat!("Takes [`", stringify!($key), "`] as a key and returns [`", stringify!($value), "`].")]
            #[derive(Debug)]
            pub struct $table_name;

            impl Table for $table_name {
                const NAME: &'static str = stringify!($table_name);
                type Key = $key;
                type Value = $value;
            }

            $(
                dupsort!($table_name, $key_type2);
            )?

            impl std::fmt::Display for $table_name {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    write!(f, "{}", stringify!($table_name))
                }
            }
       )*
    };
}

/// Macro to declare duplicate key value table.
#[macro_export]
macro_rules! dupsort {
    ($table_name:ident, $subkey:ty) => {
        impl DupSort for $table_name {
            type SubKey = $subkey;
        }
    };
}

define_tables_enum! {[
    (Headers, TableType::Table),
    (BlockHashes, TableType::Table),
    (BlockNumbers, TableType::Table),
    (BlockBodyIndices, TableType::Table),
    (BlockStatusses, TableType::Table),
    (TxNumbers, TableType::Table),
    (TxBlocks, TableType::Table),
    (TxHashes, TableType::Table),
    (TxTraces, TableType::Table),
    (Transactions, TableType::Table),
    (Receipts, TableType::Table),
    (CompiledClassHashes, TableType::Table),
    (CompiledClasses, TableType::Table),
    (SierraClasses, TableType::Table),
    (ContractInfo, TableType::Table),
    (ContractStorage, TableType::DupSort),
    (ClassDeclarationBlock, TableType::Table),
    (ClassDeclarations, TableType::DupSort),
    (ContractInfoChangeSet, TableType::Table),
    (NonceChangeHistory, TableType::DupSort),
    (ClassChangeHistory, TableType::DupSort),
    (StorageChangeHistory, TableType::DupSort),
    (StorageChangeSet, TableType::Table),
    (MessagingCheckpointBlock, TableType::Table),
    (MessagingCheckpointNonce, TableType::Table),
    (MessagingMessageNonceMapping, TableType::Table),
    (MessagingCheckpointIndex, TableType::Table)
]}

tables! {
    /// Store canonical block headers
    Headers: (BlockNumber) => Header,
    /// Stores block hashes according to its block number
    BlockHashes: (BlockNumber) => BlockHash,
    /// Stores block numbers according to its block hash
    BlockNumbers: (BlockHash) => BlockNumber,
    /// Stores block finality status according to its block number
    BlockStatusses: (BlockNumber) => FinalityStatus,
    /// Block number to its body indices which stores the tx number of
    /// the first tx in the block and the number of txs in the block.
    BlockBodyIndices: (BlockNumber) => StoredBlockBodyIndices,
    /// Transaction number based on its hash
    TxNumbers: (TxHash) => TxNumber,
    /// Transaction hash based on its number
    TxHashes: (TxNumber) => TxHash,
    /// Store canonical transactions
    Transactions: (TxNumber) => Tx,
    /// Stores the block number of a transaction.
    TxBlocks: (TxNumber) => BlockNumber,
    /// Stores the transaction's traces.
    TxTraces: (TxNumber) => TxExecInfo,
    /// Store transaction receipts
    Receipts: (TxNumber) => Receipt,
    /// Store compiled classes
    CompiledClassHashes: (ClassHash) => CompiledClassHash,
    /// Store compiled contract classes according to its compiled class hash
    CompiledClasses: (ClassHash) => CompiledClass,
    /// Store Sierra classes according to its class hash
    SierraClasses: (ClassHash) => FlattenedSierraClass,
    /// Store contract information according to its contract address
    ContractInfo: (ContractAddress) => GenericContractInfo,
    /// Store contract storage
    ContractStorage: (ContractAddress, StorageKey) => StorageEntry,


    /// Stores the block number where the class hash was declared.
    ClassDeclarationBlock: (ClassHash) => BlockNumber,
    /// Stores the list of class hashes according to the block number it was declared in.
    ClassDeclarations: (BlockNumber, ClassHash) => ClassHash,

    /// Generic contract info change set.
    ///
    /// Stores the list of blocks where the contract info (nonce / class hash) has changed.
    ContractInfoChangeSet: (ContractAddress) => ContractInfoChangeList,
    /// Contract nonce changes by block.
    NonceChangeHistory: (BlockNumber, ContractAddress) => ContractNonceChange,
    /// Contract class hash changes by block.
    ClassChangeHistory: (BlockNumber, ContractAddress) => ContractClassChange,

    /// storage change set
    StorageChangeSet: (ContractStorageKey) => BlockList,
    /// Account storage change set
    StorageChangeHistory: (BlockNumber, ContractStorageKey) => ContractStorageEntry,

    /// Stores the block number related to messaging service
    MessagingCheckpointBlock: (MessagingCheckpointId) => BlockNumber,

    /// Stores the nonce related to messaging service
    MessagingCheckpointNonce: (MessagingCheckpointId) => Nonce,

    /// Map a message hash to a message nonce
    MessagingMessageNonceMapping: (TxHash) => Nonce,

    /// Stores the index of the messaging service
    MessagingCheckpointIndex: (MessagingCheckpointId) => u64

}

#[cfg(test)]
mod tests {

    #[test]
    fn test_tables() {
        use super::*;

        assert_eq!(Tables::ALL.len(), NUM_TABLES);
        assert_eq!(Tables::ALL[0].name(), Headers::NAME);
        assert_eq!(Tables::ALL[1].name(), BlockHashes::NAME);
        assert_eq!(Tables::ALL[2].name(), BlockNumbers::NAME);
        assert_eq!(Tables::ALL[3].name(), BlockBodyIndices::NAME);
        assert_eq!(Tables::ALL[4].name(), BlockStatusses::NAME);
        assert_eq!(Tables::ALL[5].name(), TxNumbers::NAME);
        assert_eq!(Tables::ALL[6].name(), TxBlocks::NAME);
        assert_eq!(Tables::ALL[7].name(), TxHashes::NAME);
        assert_eq!(Tables::ALL[8].name(), TxTraces::NAME);
        assert_eq!(Tables::ALL[9].name(), Transactions::NAME);
        assert_eq!(Tables::ALL[10].name(), Receipts::NAME);
        assert_eq!(Tables::ALL[11].name(), CompiledClassHashes::NAME);
        assert_eq!(Tables::ALL[12].name(), CompiledClasses::NAME);
        assert_eq!(Tables::ALL[13].name(), SierraClasses::NAME);
        assert_eq!(Tables::ALL[14].name(), ContractInfo::NAME);
        assert_eq!(Tables::ALL[15].name(), ContractStorage::NAME);
        assert_eq!(Tables::ALL[16].name(), ClassDeclarationBlock::NAME);
        assert_eq!(Tables::ALL[17].name(), ClassDeclarations::NAME);
        assert_eq!(Tables::ALL[18].name(), ContractInfoChangeSet::NAME);
        assert_eq!(Tables::ALL[19].name(), NonceChangeHistory::NAME);
        assert_eq!(Tables::ALL[20].name(), ClassChangeHistory::NAME);
        assert_eq!(Tables::ALL[21].name(), StorageChangeHistory::NAME);
        assert_eq!(Tables::ALL[22].name(), StorageChangeSet::NAME);
        assert_eq!(Tables::ALL[23].name(), MessagingCheckpointBlock::NAME);
        assert_eq!(Tables::ALL[24].name(), MessagingCheckpointNonce::NAME);
        assert_eq!(Tables::ALL[25].name(), MessagingMessageNonceMapping::NAME);
        assert_eq!(Tables::ALL[26].name(), MessagingCheckpointIndex::NAME);

        assert_eq!(Tables::Headers.table_type(), TableType::Table);
        assert_eq!(Tables::BlockHashes.table_type(), TableType::Table);
        assert_eq!(Tables::BlockNumbers.table_type(), TableType::Table);
        assert_eq!(Tables::BlockBodyIndices.table_type(), TableType::Table);
        assert_eq!(Tables::BlockStatusses.table_type(), TableType::Table);
        assert_eq!(Tables::TxNumbers.table_type(), TableType::Table);
        assert_eq!(Tables::TxBlocks.table_type(), TableType::Table);
        assert_eq!(Tables::TxHashes.table_type(), TableType::Table);
        assert_eq!(Tables::TxTraces.table_type(), TableType::Table);
        assert_eq!(Tables::Transactions.table_type(), TableType::Table);
        assert_eq!(Tables::Receipts.table_type(), TableType::Table);
        assert_eq!(Tables::CompiledClassHashes.table_type(), TableType::Table);
        assert_eq!(Tables::CompiledClasses.table_type(), TableType::Table);
        assert_eq!(Tables::SierraClasses.table_type(), TableType::Table);
        assert_eq!(Tables::ContractInfo.table_type(), TableType::Table);
        assert_eq!(Tables::ContractStorage.table_type(), TableType::DupSort);
        assert_eq!(Tables::ClassDeclarationBlock.table_type(), TableType::Table);
        assert_eq!(Tables::ClassDeclarations.table_type(), TableType::DupSort);
        assert_eq!(Tables::ContractInfoChangeSet.table_type(), TableType::Table);
        assert_eq!(Tables::NonceChangeHistory.table_type(), TableType::DupSort);
        assert_eq!(Tables::ClassChangeHistory.table_type(), TableType::DupSort);
        assert_eq!(Tables::StorageChangeHistory.table_type(), TableType::DupSort);
        assert_eq!(Tables::StorageChangeSet.table_type(), TableType::Table);
        assert_eq!(Tables::MessagingCheckpointBlock.table_type(), TableType::Table);
        assert_eq!(Tables::MessagingCheckpointNonce.table_type(), TableType::Table);
        assert_eq!(Tables::MessagingMessageNonceMapping.table_type(), TableType::Table);
        assert_eq!(Tables::MessagingCheckpointIndex.table_type(), TableType::Table);
    }

    use katana_primitives::block::{BlockHash, BlockNumber, FinalityStatus, Header};
    use katana_primitives::class::{ClassHash, CompiledClass, CompiledClassHash};
    use katana_primitives::contract::{ContractAddress, GenericContractInfo};
    use katana_primitives::fee::TxFeeInfo;
    use katana_primitives::receipt::{InvokeTxReceipt, Receipt};
    use katana_primitives::trace::TxExecInfo;
    use katana_primitives::transaction::{InvokeTx, Tx, TxHash, TxNumber};
    use starknet::core::types::PriceUnit;
    use starknet::macros::felt;

    use crate::codecs::{Compress, Decode, Decompress, Encode};
    use crate::models::block::StoredBlockBodyIndices;
    use crate::models::contract::{
        ContractClassChange, ContractInfoChangeList, ContractNonceChange,
    };
    use crate::models::list::BlockList;
    use crate::models::storage::{ContractStorageEntry, ContractStorageKey, StorageEntry};
    use crate::tables::Tables::{MessagingCheckpointBlock, MessagingCheckpointNonce};

    macro_rules! assert_key_encode_decode {
	    { $( ($name:ty, $key:expr) ),* } => {
			$(
				{
					let key: $name = $key;
					let encoded = key.encode();
					let decoded = <$name as Decode>::decode(encoded.as_slice()).expect("decode failed");
					assert_eq!($key, decoded);
				}
			)*
		};
	}

    macro_rules! assert_value_compress_decompress {
		{ $( ($name:ty, $value:expr) ),* } => {
			$(
				{
					let value: $name = $value;
					let compressed = value.compress();
					let decompressed = <$name as Decompress>::decompress(compressed.as_slice()).expect("decode failed");
					assert_eq!($value, decompressed);
				}
			)*
		};
	}

    // Test that all key/subkey types can be encoded and decoded
    // through the Encode and Decode traits
    #[test]
    fn test_key_encode_decode() {
        assert_key_encode_decode! {
            (BlockNumber, 100),
            (BlockHash, felt!("0x123456789")),
            (TxHash, felt!("0x123456789")),
            (TxNumber, 100),
            (ClassHash, felt!("0x123456789")),
            (ContractAddress, ContractAddress(felt!("0x123456789"))),
            (ContractStorageKey, ContractStorageKey { contract_address : ContractAddress(felt!("0x123456789")), key : felt!("0x123456789")})
        }
    }

    // Test that all value types can be compressed and decompressed
    // through the Compress and Decompress traits
    #[test]
    fn test_value_compress_decompress() {
        assert_value_compress_decompress! {
            (Header, Header::default()),
            (BlockHash, BlockHash::default()),
            (BlockNumber, BlockNumber::default()),
            (FinalityStatus, FinalityStatus::AcceptedOnL1),
            (StoredBlockBodyIndices, StoredBlockBodyIndices::default()),
            (TxNumber, 77),
            (TxHash, felt!("0x123456789")),
            (Tx, Tx::Invoke(InvokeTx::V1(Default::default()))),
            (BlockNumber, 99),
            (TxExecInfo, TxExecInfo::default()),
            (CompiledClassHash, felt!("211")),
            (CompiledClass, CompiledClass::Deprecated(Default::default())),
            (GenericContractInfo, GenericContractInfo::default()),
            (StorageEntry, StorageEntry::default()),
            (ContractInfoChangeList, ContractInfoChangeList::default()),
            (ContractNonceChange, ContractNonceChange::default()),
            (ContractClassChange, ContractClassChange::default()),
            (BlockList, BlockList::default()),
            (ContractStorageEntry, ContractStorageEntry::default()),
            (Receipt, Receipt::Invoke(InvokeTxReceipt {
                        revert_error: None,
                        events: Vec::new(),
                        messages_sent: Vec::new(),
                        execution_resources: Default::default(),
                        fee: TxFeeInfo { gas_consumed: 0, gas_price: 0, overall_fee: 0, unit: PriceUnit::Wei },
                    }))
        }
    }
}
