use katana_primitives::block::{BlockHash, BlockNumber, FinalityStatus, Header};
use katana_primitives::contract::{
    ClassHash, CompiledClass, CompiledClassHash, ContractAddress, FlattenedSierraClass,
    GenericContractInfo, StorageKey,
};
use katana_primitives::receipt::Receipt;
use katana_primitives::transaction::{Tx, TxHash, TxNumber};

use crate::codecs::{Compress, Decode, Decompress, Encode};
use crate::models::block::StoredBlockBodyIndices;
use crate::models::contract::{ContractClassChange, ContractInfoChangeList, ContractNonceChange};
use crate::models::storage::{
    ContractStorageEntry, ContractStorageKey, StorageEntry, StorageEntryChangeList,
};

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

pub const NUM_TABLES: usize = 22;

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
    (NonceChanges, TableType::DupSort),
    (ContractClassChanges, TableType::DupSort),
    (StorageChanges, TableType::DupSort),
    (StorageChangeSet, TableType::DupSort)
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
    NonceChanges: (BlockNumber, ContractAddress) => ContractNonceChange,
    /// Contract class hash changes by block.
    ContractClassChanges: (BlockNumber, ContractAddress) => ContractClassChange,

    /// storage change set
    StorageChangeSet: (ContractAddress, StorageKey) => StorageEntryChangeList,
    /// Account storage change set
    StorageChanges: (BlockNumber, ContractStorageKey) => ContractStorageEntry

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
        assert_eq!(Tables::ALL[8].name(), Transactions::NAME);
        assert_eq!(Tables::ALL[9].name(), Receipts::NAME);
        assert_eq!(Tables::ALL[10].name(), CompiledClassHashes::NAME);
        assert_eq!(Tables::ALL[11].name(), CompiledClasses::NAME);
        assert_eq!(Tables::ALL[12].name(), SierraClasses::NAME);
        assert_eq!(Tables::ALL[13].name(), ContractInfo::NAME);
        assert_eq!(Tables::ALL[14].name(), ContractStorage::NAME);
        assert_eq!(Tables::ALL[15].name(), ClassDeclarationBlock::NAME);
        assert_eq!(Tables::ALL[16].name(), ClassDeclarations::NAME);
        assert_eq!(Tables::ALL[17].name(), ContractInfoChangeSet::NAME);
        assert_eq!(Tables::ALL[18].name(), NonceChanges::NAME);
        assert_eq!(Tables::ALL[19].name(), ContractClassChanges::NAME);
        assert_eq!(Tables::ALL[20].name(), StorageChanges::NAME);
        assert_eq!(Tables::ALL[21].name(), StorageChangeSet::NAME);
    }
}
