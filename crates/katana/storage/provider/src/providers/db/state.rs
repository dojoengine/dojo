use std::cmp::Ordering;

use katana_db::mdbx::{self};
use katana_db::models::contract::ContractInfoChangeList;
use katana_db::models::storage::{ContractStorageKey, StorageEntry};
use katana_db::tables;
use katana_primitives::block::BlockNumber;
use katana_primitives::contract::{
    ClassHash, CompiledClass, CompiledClassHash, ContractAddress, FlattenedSierraClass,
    GenericContractInfo, Nonce, StorageKey, StorageValue,
};

use super::DbProvider;
use crate::error::ProviderError;
use crate::traits::contract::{ContractClassProvider, ContractClassWriter};
use crate::traits::state::{StateProvider, StateWriter};
use crate::ProviderResult;

impl StateWriter for DbProvider {
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
            let mut cursor = db_tx.cursor::<tables::ContractStorage>()?;
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
    fn set_class(&self, hash: ClassHash, class: CompiledClass) -> ProviderResult<()> {
        self.0.update(move |db_tx| -> ProviderResult<()> {
            db_tx.put::<tables::CompiledClasses>(hash, class)?;
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

    fn set_sierra_class(
        &self,
        hash: ClassHash,
        sierra: FlattenedSierraClass,
    ) -> ProviderResult<()> {
        self.0.update(move |db_tx| -> ProviderResult<()> {
            db_tx.put::<tables::SierraClasses>(hash, sierra)?;
            Ok(())
        })?
    }
}

/// A state provider that provides the latest states from the database.
pub(super) struct LatestStateProvider(mdbx::tx::TxRO);

impl LatestStateProvider {
    pub fn new(tx: mdbx::tx::TxRO) -> Self {
        Self(tx)
    }
}

impl ContractClassProvider for LatestStateProvider {
    fn class(&self, hash: ClassHash) -> ProviderResult<Option<CompiledClass>> {
        let class = self.0.get::<tables::CompiledClasses>(hash)?;
        Ok(class)
    }

    fn compiled_class_hash_of_class_hash(
        &self,
        hash: ClassHash,
    ) -> ProviderResult<Option<CompiledClassHash>> {
        let hash = self.0.get::<tables::CompiledClassHashes>(hash)?;
        Ok(hash)
    }

    fn sierra_class(&self, hash: ClassHash) -> ProviderResult<Option<FlattenedSierraClass>> {
        let class = self.0.get::<tables::SierraClasses>(hash)?;
        Ok(class)
    }
}

impl StateProvider for LatestStateProvider {
    fn nonce(&self, address: ContractAddress) -> ProviderResult<Option<Nonce>> {
        let info = self.0.get::<tables::ContractInfo>(address)?;
        Ok(info.map(|info| info.nonce))
    }

    fn class_hash_of_contract(
        &self,
        address: ContractAddress,
    ) -> ProviderResult<Option<katana_primitives::contract::ClassHash>> {
        let info = self.0.get::<tables::ContractInfo>(address)?;
        Ok(info.map(|info| info.class_hash))
    }

    fn storage(
        &self,
        address: ContractAddress,
        storage_key: StorageKey,
    ) -> ProviderResult<Option<StorageValue>> {
        let mut cursor = self.0.cursor::<tables::ContractStorage>()?;
        let entry = cursor.seek_by_key_subkey(address, storage_key)?;
        match entry {
            Some(entry) if entry.key == storage_key => Ok(Some(entry.value)),
            _ => Ok(None),
        }
    }
}

/// A historical state provider.
pub(super) struct HistoricalStateProvider {
    /// The database transaction used to read the database.
    tx: mdbx::tx::TxRO,
    /// The block number of the state.
    block_number: u64,
}

impl HistoricalStateProvider {
    pub fn new(tx: mdbx::tx::TxRO, block_number: u64) -> Self {
        Self { tx, block_number }
    }

    // This looks ugly but it works and I will most likely forget how it works
    // if I don't document it. But im lazy.
    fn recent_block_change_relative_to_pinned_block_num(
        block_number: BlockNumber,
        block_list: &[BlockNumber],
    ) -> Option<BlockNumber> {
        if block_list.first().is_some_and(|num| block_number < *num) {
            return None;
        }

        // if the pinned block number is smaller than the first block number in the list,
        // then that means there is no change happening before the pinned block number.
        let pos = {
            if let Some(pos) = block_list.last().and_then(|num| {
                if block_number >= *num { Some(block_list.len() - 1) } else { None }
            }) {
                Some(pos)
            } else {
                block_list.iter().enumerate().find_map(|(i, num)| match block_number.cmp(num) {
                    Ordering::Equal => Some(i),
                    Ordering::Greater => None,
                    Ordering::Less => {
                        if i == 0 || block_number == 0 {
                            None
                        } else {
                            Some(i - 1)
                        }
                    }
                })
            }
        }?;

        block_list.get(pos).copied()
    }
}

impl ContractClassProvider for HistoricalStateProvider {
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

    fn class(&self, hash: ClassHash) -> ProviderResult<Option<CompiledClass>> {
        if self.compiled_class_hash_of_class_hash(hash)?.is_some() {
            let contract = self.tx.get::<tables::CompiledClasses>(hash)?;
            Ok(contract)
        } else {
            Ok(None)
        }
    }

    fn sierra_class(&self, hash: ClassHash) -> ProviderResult<Option<FlattenedSierraClass>> {
        if self.compiled_class_hash_of_class_hash(hash)?.is_some() {
            self.tx.get::<tables::SierraClasses>(hash).map_err(|e| e.into())
        } else {
            Ok(None)
        }
    }
}

impl StateProvider for HistoricalStateProvider {
    fn nonce(&self, address: ContractAddress) -> ProviderResult<Option<Nonce>> {
        let change_list = self.tx.get::<tables::ContractInfoChangeSet>(address)?;

        if let Some(num) = change_list.and_then(|entry| {
            Self::recent_block_change_relative_to_pinned_block_num(
                self.block_number,
                &entry.nonce_change_list,
            )
        }) {
            let mut cursor = self.tx.cursor::<tables::NonceChanges>()?;
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

        if let Some(num) = change_list.and_then(|entry| {
            Self::recent_block_change_relative_to_pinned_block_num(
                self.block_number,
                &entry.class_change_list,
            )
        }) {
            let mut cursor = self.tx.cursor::<tables::ContractClassChanges>()?;
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
        let mut cursor = self.tx.cursor::<tables::StorageChangeSet>()?;

        if let Some(num) = cursor.seek_by_key_subkey(address, storage_key)?.and_then(|entry| {
            Self::recent_block_change_relative_to_pinned_block_num(
                self.block_number,
                &entry.block_list,
            )
        }) {
            let mut cursor = self.tx.cursor::<tables::StorageChanges>()?;
            let sharded_key = ContractStorageKey { contract_address: address, key: storage_key };

            let entry = cursor.seek_by_key_subkey(num, sharded_key)?.ok_or(
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

#[cfg(test)]
mod tests {
    use super::HistoricalStateProvider;

    const BLOCK_LIST: [u64; 5] = [1, 2, 5, 6, 10];

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
        assert_eq!(
            HistoricalStateProvider::recent_block_change_relative_to_pinned_block_num(
                block_num,
                &BLOCK_LIST,
            ),
            expected_block_num
        );
    }
}
