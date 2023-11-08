use katana_primitives::block::{Header, StateUpdate};
use katana_primitives::contract::{
    ClassHash, CompiledClassHash, ContractAddress, GenericContractInfo, SierraClass, StorageKey,
    StorageValue,
};
use katana_primitives::transaction::{Receipt, Transaction, TxHash};
use serde::{Deserialize, Serialize};

use super::models::{StoredBlockBodyIndices, TxNumber};
use crate::codecs::{Compress, Decode, Decompress, Encode};

pub trait Key: Encode + Decode + Serialize + for<'a> Deserialize<'a> + Clone {}
pub trait Value: Compress + Decompress {}

impl<T> Key for T where T: Serialize + for<'a> Deserialize<'a> + Clone {}
impl<T> Value for T where T: Serialize + for<'a> Deserialize<'a> {}

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

pub const NUM_TABLES: usize = 9;

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
                        return Err("Unknown table".to_string())
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
    (BlockBodyIndices, TableType::Table),
    (Transactions, TableType::Table),
    (Receipts, TableType::Table),
    (StateUpdates, TableType::Table),
    (CompiledContractClasses, TableType::Table),
    (SierraClasses, TableType::Table),
    (ContractInfo, TableType::Table),
    (ContractStorage, TableType::DupSort)
]}

tables! {
    /// Store canonical block headers
    Headers: (u64) => Header,
    /// Store block headers
    BlockBodyIndices: (u64) => StoredBlockBodyIndices,
    /// Store canonical transactions
    TxHashNumber: (TxHash) => TxNumber,
    /// Store canonical transactions
    Transactions: (TxNumber) => Transaction,
    /// Store transaction receipts
    Receipts: (TxNumber) => Receipt,
    /// Store block state updates
    StateUpdates: (u64) => StateUpdate,
    /// Store compiled classes
    CompiledClassHashes: (ClassHash) => CompiledClassHash,
    /// Store compiled contract classes according to its compiled class hash
    CompiledContractClasses: (CompiledClassHash) => u64,
    /// Store Sierra classes
    SierraClasses: (ClassHash) => SierraClass,
    /// Store contract information
    ContractInfo: (ContractAddress) => GenericContractInfo,
    /// Store contract storage
    ContractStorage: (ContractAddress, StorageKey) => StorageValue
}
