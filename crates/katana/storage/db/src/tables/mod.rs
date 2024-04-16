pub mod v0;
mod v1;

use std::fmt::{Debug, Display};
use std::str::FromStr;

pub use self::v1::*;
use crate::codecs::{Compress, Decode, Decompress, Encode};

pub trait Key: Encode + Decode + Clone + std::fmt::Debug {}
pub trait Value: Compress + Decompress + std::fmt::Debug {}

impl<T> Key for T where T: Encode + Decode + Clone + std::fmt::Debug {}
impl<T> Value for T where T: Compress + Decompress + std::fmt::Debug {}

/// Trait for defining the database schema.
///
/// This trait is useful for us to maintain the schema for different database versions.
pub trait Schema: Debug + Display + FromStr + 'static {
    /// The version of the schema.
    const VERSION: u32;
    /// Returns the list of tables in the schema.
    fn all() -> &'static [Self];
    /// The name of the tables.
    fn name(&self) -> &str;
    /// The type of the tables.
    fn table_type(&self) -> TableType;

    fn index<T: Table>() -> Option<usize>;
}

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

/// Macro to declare database tables based on the [Database$name] trait.
#[macro_export]
macro_rules! define_schema_enum {
    { $ver:expr, $name:ident, [$(($table:ident, $type:expr)),*] } => {
        #[derive(Debug, PartialEq, Copy, Clone)]
        pub enum $name {
            $(
                $table,
            )*
        }

        impl Schema for $name {
        	const VERSION: u32 = $ver;

            fn all() -> &'static [Self] { &[$($name::$table,)*] }

            fn name(&self) -> &str {
                match self {
                    $($name::$table => {
                        $table::NAME
                    },)*
                }
            }

            fn table_type(&self) -> TableType {
                match self {
                    $($name::$table => {
                        $type
                    },)*
                }
            }

            fn index<T: Table>() -> Option<usize> {
	            match T::NAME {
	                $($table::NAME => {
	                    Some($name::$table as usize)
	                },)*
	                _ => {
	                    None
	                }
	            }
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.name())
            }
        }

        impl std::str::FromStr for $name {
            type Err = String;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match s {
                    $($table::NAME => {
                        return Ok($name::$table)
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
