use std::fmt::Debug;

use katana_primitives::block::{BlockHash, BlockNumber, FinalityStatus, Header};
use katana_primitives::class::{ClassHash, CompiledClass, CompiledClassHash, FlattenedSierraClass};
use katana_primitives::contract::{ContractAddress, GenericContractInfo, StorageKey};
use katana_primitives::receipt::Receipt;
use katana_primitives::trace::TxExecInfo;
use katana_primitives::transaction::{Tx, TxHash, TxNumber};

use super::{DupSort, Schema, Table, TableType};
use crate::models::block::StoredBlockBodyIndices;
use crate::models::contract::{ContractClassChange, ContractInfoChangeList, ContractNonceChange};
use crate::models::list::BlockList;
use crate::models::storage::{ContractStorageEntry, ContractStorageKey, StorageEntry};
use crate::{define_schema_enum, dupsort, tables};

pub const NUM_TABLES: usize = 23;

define_schema_enum! {
    1,
    SchemaV1,
    [(Headers, TableType::Table),
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
    (StorageChangeSet, TableType::Table)
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
    StorageChangeHistory: (BlockNumber, ContractStorageKey) => ContractStorageEntry

}

#[cfg(test)]
mod tests {

    #[test]
    fn test_tables() {
        use super::*;

        assert_eq!(SchemaV1::all().len(), NUM_TABLES);
        assert_eq!(SchemaV1::all()[0].name(), Headers::NAME);
        assert_eq!(SchemaV1::all()[1].name(), BlockHashes::NAME);
        assert_eq!(SchemaV1::all()[2].name(), BlockNumbers::NAME);
        assert_eq!(SchemaV1::all()[3].name(), BlockBodyIndices::NAME);
        assert_eq!(SchemaV1::all()[4].name(), BlockStatusses::NAME);
        assert_eq!(SchemaV1::all()[5].name(), TxNumbers::NAME);
        assert_eq!(SchemaV1::all()[6].name(), TxBlocks::NAME);
        assert_eq!(SchemaV1::all()[7].name(), TxTraces::NAME);
        assert_eq!(SchemaV1::all()[8].name(), TxHashes::NAME);
        assert_eq!(SchemaV1::all()[9].name(), Transactions::NAME);
        assert_eq!(SchemaV1::all()[10].name(), Receipts::NAME);
        assert_eq!(SchemaV1::all()[11].name(), CompiledClassHashes::NAME);
        assert_eq!(SchemaV1::all()[12].name(), CompiledClasses::NAME);
        assert_eq!(SchemaV1::all()[13].name(), SierraClasses::NAME);
        assert_eq!(SchemaV1::all()[14].name(), ContractInfo::NAME);
        assert_eq!(SchemaV1::all()[15].name(), ContractStorage::NAME);
        assert_eq!(SchemaV1::all()[16].name(), ClassDeclarationBlock::NAME);
        assert_eq!(SchemaV1::all()[17].name(), ClassDeclarations::NAME);
        assert_eq!(SchemaV1::all()[18].name(), ContractInfoChangeSet::NAME);
        assert_eq!(SchemaV1::all()[19].name(), NonceChangeHistory::NAME);
        assert_eq!(SchemaV1::all()[20].name(), ClassChangeHistory::NAME);
        assert_eq!(SchemaV1::all()[21].name(), StorageChangeHistory::NAME);
        assert_eq!(SchemaV1::all()[22].name(), StorageChangeSet::NAME);

        assert_eq!(SchemaV1::Headers.table_type(), TableType::Table);
        assert_eq!(SchemaV1::BlockHashes.table_type(), TableType::Table);
        assert_eq!(SchemaV1::BlockNumbers.table_type(), TableType::Table);
        assert_eq!(SchemaV1::BlockBodyIndices.table_type(), TableType::Table);
        assert_eq!(SchemaV1::BlockStatusses.table_type(), TableType::Table);
        assert_eq!(SchemaV1::TxNumbers.table_type(), TableType::Table);
        assert_eq!(SchemaV1::TxBlocks.table_type(), TableType::Table);
        assert_eq!(SchemaV1::TxHashes.table_type(), TableType::Table);
        assert_eq!(SchemaV1::TxTraces.table_type(), TableType::Table);
        assert_eq!(SchemaV1::Transactions.table_type(), TableType::Table);
        assert_eq!(SchemaV1::Receipts.table_type(), TableType::Table);
        assert_eq!(SchemaV1::CompiledClassHashes.table_type(), TableType::Table);
        assert_eq!(SchemaV1::CompiledClasses.table_type(), TableType::Table);
        assert_eq!(SchemaV1::SierraClasses.table_type(), TableType::Table);
        assert_eq!(SchemaV1::ContractInfo.table_type(), TableType::Table);
        assert_eq!(SchemaV1::ContractStorage.table_type(), TableType::DupSort);
        assert_eq!(SchemaV1::ClassDeclarationBlock.table_type(), TableType::Table);
        assert_eq!(SchemaV1::ClassDeclarations.table_type(), TableType::DupSort);
        assert_eq!(SchemaV1::ContractInfoChangeSet.table_type(), TableType::Table);
        assert_eq!(SchemaV1::NonceChangeHistory.table_type(), TableType::DupSort);
        assert_eq!(SchemaV1::ClassChangeHistory.table_type(), TableType::DupSort);
        assert_eq!(SchemaV1::StorageChangeHistory.table_type(), TableType::DupSort);
        assert_eq!(SchemaV1::StorageChangeSet.table_type(), TableType::Table);
    }

    use katana_primitives::block::{BlockHash, BlockNumber, FinalityStatus, Header};
    use katana_primitives::class::{ClassHash, CompiledClass, CompiledClassHash};
    use katana_primitives::contract::{ContractAddress, GenericContractInfo};
    use katana_primitives::receipt::Receipt;
    use katana_primitives::trace::TxExecInfo;
    use katana_primitives::transaction::{InvokeTx, Tx, TxHash, TxNumber};
    use starknet::macros::felt;

    use crate::codecs::{Compress, Decode, Decompress, Encode};
    use crate::models::block::StoredBlockBodyIndices;
    use crate::models::contract::{
        ContractClassChange, ContractInfoChangeList, ContractNonceChange,
    };
    use crate::models::list::BlockList;
    use crate::models::storage::{ContractStorageEntry, ContractStorageKey, StorageEntry};

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
            (Receipt, Receipt::Invoke(Default::default())),
            (CompiledClassHash, felt!("211")),
            (CompiledClass, CompiledClass::Deprecated(Default::default())),
            (GenericContractInfo, GenericContractInfo::default()),
            (StorageEntry, StorageEntry::default()),
            (ContractInfoChangeList, ContractInfoChangeList::default()),
            (ContractNonceChange, ContractNonceChange::default()),
            (ContractClassChange, ContractClassChange::default()),
            (BlockList, BlockList::default()),
            (ContractStorageEntry, ContractStorageEntry::default())
        }
    }
}
