use core::fmt;

use katana_db::abstraction::{Database, DbCursorMut, DbDupSortCursor, DbTx, DbTxMut};
use katana_db::models::contract::ContractInfoChangeList;
use katana_db::models::list::BlockList;
use katana_db::models::storage::{ContractStorageKey, StorageEntry};
use katana_db::tables;
use katana_primitives::block::BlockNumber;
use katana_primitives::class::{ClassHash, CompiledClass, CompiledClassHash, ContractClass};
use katana_primitives::contract::{
    ContractAddress, GenericContractInfo, Nonce, StorageKey, StorageValue,
};

use super::DbProvider;
use crate::error::ProviderError;
use crate::traits::contract::{ContractClassProvider, ContractClassWriter};
use crate::traits::state::{StateProvider, StateWriter};
use crate::ProviderResult;

impl<Db: Database> StateWriter for DbProvider<Db> {
    fn set_nonce(&self, address: ContractAddress, nonce: Nonce) -> ProviderResult<()> {
        self.0.update(move |db_tx| -> ProviderResult<()> {
            let value = if let Some(info) = db_tx.get::<tables::ContractInfo>(address)? {
                GenericContractInfo { nonce, ..info }
            } else {
                GenericContractInfo { nonce, ..Default::default() }
            };
            db_tx.put::<tables::ContractInfo>(address, value)?;
            Ok(())
        })?
    }

    fn set_storage(
        &self,
        address: ContractAddress,
        storage_key: StorageKey,
        storage_value: StorageValue,
    ) -> ProviderResult<()> {
        self.0.update(move |db_tx| -> ProviderResult<()> {
            let mut cursor = db_tx.cursor_dup_mut::<tables::ContractStorage>()?;
            let entry = cursor.seek_by_key_subkey(address, storage_key)?;

            match entry {
                Some(entry) if entry.key == storage_key => {
                    cursor.delete_current()?;
                }
                _ => {}
            }

            cursor.upsert(address, StorageEntry { key: storage_key, value: storage_value })?;
            Ok(())
        })?
    }

    fn set_class_hash_of_contract(
        &self,
        address: ContractAddress,
        class_hash: ClassHash,
    ) -> ProviderResult<()> {
        self.0.update(move |db_tx| -> ProviderResult<()> {
            let value = if let Some(info) = db_tx.get::<tables::ContractInfo>(address)? {
                GenericContractInfo { class_hash, ..info }
            } else {
                GenericContractInfo { class_hash, ..Default::default() }
            };
            db_tx.put::<tables::ContractInfo>(address, value)?;
            Ok(())
        })?
    }
}

impl ContractClassWriter for DbProvider {
    fn set_class(&self, hash: ClassHash, class: ContractClass) -> ProviderResult<()> {
        self.0.update(move |db_tx| -> ProviderResult<()> {
            match class {
                ContractClass::Class(sierra) => {
                    db_tx.put::<tables::SierraClasses>(hash, sierra)?;
                }
                ContractClass::Legacy(class) => {
                    db_tx.put::<tables::CompiledClasses>(hash, CompiledClass::Legacy(class))?;
                }
            }
            Ok(())
        })?
    }

    fn set_compiled_class_hash_of_class_hash(
        &self,
        hash: ClassHash,
        compiled_hash: CompiledClassHash,
    ) -> ProviderResult<()> {
        self.0.update(move |db_tx| -> ProviderResult<()> {
            db_tx.put::<tables::CompiledClassHashes>(hash, compiled_hash)?;
            Ok(())
        })?
    }

    fn set_compiled_class(&self, hash: ClassHash, class: CompiledClass) -> ProviderResult<()> {
        self.0.update(move |db_tx| -> ProviderResult<()> {
            db_tx.put::<tables::CompiledClasses>(hash, class)?;
            Ok(())
        })?
    }
}

/// A state provider that provides the latest states from the database.
#[derive(Debug)]
pub(super) struct LatestStateProvider<Tx: DbTx>(Tx);

impl<Tx: DbTx> LatestStateProvider<Tx> {
    pub fn new(tx: Tx) -> Self {
        Self(tx)
    }
}

impl<Tx> ContractClassProvider for LatestStateProvider<Tx>
where
    Tx: DbTx + Send + Sync,
{
    fn class(&self, hash: ClassHash) -> ProviderResult<Option<ContractClass>> {
        match self.0.get::<tables::CompiledClasses>(hash)? {
            Some(CompiledClass::Class(..)) => {
                let class = self.0.get::<tables::SierraClasses>(hash)?;
                Ok(class.map(ContractClass::Class))
            }
            Some(CompiledClass::Legacy(class)) => Ok(Some(ContractClass::Legacy(class))),
            None => Ok(None),
        }
    }

    fn compiled_class(&self, hash: ClassHash) -> ProviderResult<Option<CompiledClass>> {
        Ok(self.0.get::<tables::CompiledClasses>(hash)?)
    }

    fn compiled_class_hash_of_class_hash(
        &self,
        hash: ClassHash,
    ) -> ProviderResult<Option<CompiledClassHash>> {
        let hash = self.0.get::<tables::CompiledClassHashes>(hash)?;
        Ok(hash)
    }
}

impl<Tx> StateProvider for LatestStateProvider<Tx>
where
    Tx: DbTx + fmt::Debug + Send + Sync,
{
    fn nonce(&self, address: ContractAddress) -> ProviderResult<Option<Nonce>> {
        let info = self.0.get::<tables::ContractInfo>(address)?;
        Ok(info.map(|info| info.nonce))
    }

    fn class_hash_of_contract(
        &self,
        address: ContractAddress,
    ) -> ProviderResult<Option<ClassHash>> {
        let info = self.0.get::<tables::ContractInfo>(address)?;
        Ok(info.map(|info| info.class_hash))
    }

    fn storage(
        &self,
        address: ContractAddress,
        storage_key: StorageKey,
    ) -> ProviderResult<Option<StorageValue>> {
        let mut cursor = self.0.cursor_dup::<tables::ContractStorage>()?;
        let entry = cursor.seek_by_key_subkey(address, storage_key)?;
        match entry {
            Some(entry) if entry.key == storage_key => Ok(Some(entry.value)),
            _ => Ok(None),
        }
    }
}

/// A historical state provider.
#[derive(Debug)]
pub(super) struct HistoricalStateProvider<Tx: DbTx + fmt::Debug> {
    /// The database transaction used to read the database.
    tx: Tx,
    /// The block number of the state.
    block_number: u64,
}

impl<Tx: DbTx + fmt::Debug> HistoricalStateProvider<Tx> {
    pub fn new(tx: Tx, block_number: u64) -> Self {
        Self { tx, block_number }
    }
}

impl<Tx> ContractClassProvider for HistoricalStateProvider<Tx>
where
    Tx: DbTx + fmt::Debug + Send + Sync,
{
    fn class(&self, hash: ClassHash) -> ProviderResult<Option<ContractClass>> {
        if self.compiled_class_hash_of_class_hash(hash)?.is_some() {
            match self.tx.get::<tables::CompiledClasses>(hash)? {
                None => Ok(None),
                Some(CompiledClass::Class(..)) => {
                    let class = self.tx.get::<tables::SierraClasses>(hash)?;
                    Ok(class.map(ContractClass::Class))
                }
                Some(CompiledClass::Legacy(class)) => Ok(Some(ContractClass::Legacy(class))),
            }
        } else {
            Ok(None)
        }
    }

    fn compiled_class(&self, hash: ClassHash) -> ProviderResult<Option<CompiledClass>> {
        if self.compiled_class_hash_of_class_hash(hash)?.is_some() {
            Ok(self.tx.get::<tables::CompiledClasses>(hash)?)
        } else {
            Ok(None)
        }
    }

    fn compiled_class_hash_of_class_hash(
        &self,
        hash: ClassHash,
    ) -> ProviderResult<Option<CompiledClassHash>> {
        // check that the requested class hash was declared before the pinned block number
        if self
            .tx
            .get::<tables::ClassDeclarationBlock>(hash)?
            .is_some_and(|num| num <= self.block_number)
        {
            Ok(self.tx.get::<tables::CompiledClassHashes>(hash)?)
        } else {
            Ok(None)
        }
    }
}

impl<Tx> StateProvider for HistoricalStateProvider<Tx>
where
    Tx: DbTx + fmt::Debug + Send + Sync,
{
    fn nonce(&self, address: ContractAddress) -> ProviderResult<Option<Nonce>> {
        let change_list = self.tx.get::<tables::ContractInfoChangeSet>(address)?;

        if let Some(num) = change_list
            .and_then(|entry| recent_change_from_block(self.block_number, &entry.nonce_change_list))
        {
            let mut cursor = self.tx.cursor_dup::<tables::NonceChangeHistory>()?;
            let entry = cursor.seek_by_key_subkey(num, address)?.ok_or(
                ProviderError::MissingContractNonceChangeEntry {
                    block: num,
                    contract_address: address,
                },
            )?;

            if entry.contract_address == address {
                return Ok(Some(entry.nonce));
            }
        }

        Ok(None)
    }

    fn class_hash_of_contract(
        &self,
        address: ContractAddress,
    ) -> ProviderResult<Option<ClassHash>> {
        let change_list: Option<ContractInfoChangeList> =
            self.tx.get::<tables::ContractInfoChangeSet>(address)?;

        if let Some(num) = change_list
            .and_then(|entry| recent_change_from_block(self.block_number, &entry.class_change_list))
        {
            let mut cursor = self.tx.cursor_dup::<tables::ClassChangeHistory>()?;
            let entry = cursor.seek_by_key_subkey(num, address)?.ok_or(
                ProviderError::MissingContractClassChangeEntry {
                    block: num,
                    contract_address: address,
                },
            )?;

            if entry.contract_address == address {
                return Ok(Some(entry.class_hash));
            }
        }

        Ok(None)
    }

    fn storage(
        &self,
        address: ContractAddress,
        storage_key: StorageKey,
    ) -> ProviderResult<Option<StorageValue>> {
        let key = ContractStorageKey { contract_address: address, key: storage_key };
        let block_list = self.tx.get::<tables::StorageChangeSet>(key.clone())?;

        if let Some(num) =
            block_list.and_then(|list| recent_change_from_block(self.block_number, &list))
        {
            let mut cursor = self.tx.cursor_dup::<tables::StorageChangeHistory>()?;
            let entry = cursor.seek_by_key_subkey(num, key)?.ok_or(
                ProviderError::MissingStorageChangeEntry {
                    block: num,
                    storage_key,
                    contract_address: address,
                },
            )?;

            if entry.key.contract_address == address && entry.key.key == storage_key {
                return Ok(Some(entry.value));
            }
        }

        Ok(None)
    }
}

/// This is a helper function for getting the block number of the most
/// recent change that occurred relative to the given block number.
fn recent_change_from_block(
    block_number: BlockNumber,
    block_list: &BlockList,
) -> Option<BlockNumber> {
    // if the rank is 0, then it's either;
    // 1. the list is empty
    // 2. there are no prior changes occured before/at `block_number`
    let rank = block_list.rank(block_number);
    if rank == 0 { None } else { block_list.select(rank - 1) }
}

#[cfg(test)]
mod tests {
    use katana_db::models::list::BlockList;

    #[rstest::rstest]
    #[case(0, None)]
    #[case(1, Some(1))]
    #[case(3, Some(2))]
    #[case(5, Some(5))]
    #[case(9, Some(6))]
    #[case(10, Some(10))]
    #[case(11, Some(10))]
    fn position_of_most_recent_block_in_block_list(
        #[case] block_num: u64,
        #[case] expected_block_num: Option<u64>,
    ) {
        let list = BlockList::from([1, 2, 5, 6, 10]);
        let actual_block_num = super::recent_change_from_block(block_num, &list);
        assert_eq!(actual_block_num, expected_block_num);
    }
}
