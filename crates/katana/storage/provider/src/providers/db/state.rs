use std::cmp::Ordering;

use anyhow::Result;
use katana_db::mdbx::{self};
use katana_db::models::contract::{
    ContractClassChange, ContractInfoChangeList, ContractNonceChange,
};
use katana_db::models::storage::{ContractStorageEntry, ContractStorageKey, StorageEntry};
use katana_db::tables::{
    ClassDeclarationBlock, CompiledClassHashes, CompiledContractClasses, ContractClassChanges,
    ContractInfo, ContractInfoChangeSet, ContractStorage, NonceChanges, SierraClasses,
    StorageChangeSet, StorageChanges,
};
use katana_primitives::block::BlockNumber;
use katana_primitives::contract::{
    ClassHash, CompiledClassHash, CompiledContractClass, ContractAddress, FlattenedSierraClass,
    GenericContractInfo, Nonce, StorageKey, StorageValue,
};

use super::DbProvider;
use crate::traits::contract::{ContractClassProvider, ContractClassWriter};
use crate::traits::state::{StateProvider, StateWriter};

impl StateWriter for DbProvider {
    fn set_nonce(&self, address: ContractAddress, nonce: Nonce) -> Result<()> {
        self.0.update(move |db_tx| -> Result<()> {
            let value = if let Some(info) = db_tx.get::<ContractInfo>(address)? {
                GenericContractInfo { nonce, ..info }
            } else {
                GenericContractInfo { nonce, ..Default::default() }
            };
            db_tx.put::<ContractInfo>(address, value)?;
            Ok(())
        })?
    }

    fn set_storage(
        &self,
        address: ContractAddress,
        storage_key: StorageKey,
        storage_value: StorageValue,
    ) -> Result<()> {
        self.0.update(move |db_tx| -> Result<()> {
            let mut cursor = db_tx.cursor::<ContractStorage>()?;
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
    ) -> Result<()> {
        self.0.update(move |db_tx| -> Result<()> {
            let value = if let Some(info) = db_tx.get::<ContractInfo>(address)? {
                GenericContractInfo { class_hash, ..info }
            } else {
                GenericContractInfo { class_hash, ..Default::default() }
            };
            db_tx.put::<ContractInfo>(address, value)?;
            Ok(())
        })?
    }
}

impl ContractClassWriter for DbProvider {
    fn set_class(&self, hash: ClassHash, class: CompiledContractClass) -> Result<()> {
        self.0.update(move |db_tx| -> Result<()> {
            db_tx.put::<CompiledContractClasses>(hash, class.into())?;
            Ok(())
        })?
    }

    fn set_compiled_class_hash_of_class_hash(
        &self,
        hash: ClassHash,
        compiled_hash: CompiledClassHash,
    ) -> Result<()> {
        self.0.update(move |db_tx| -> Result<()> {
            db_tx.put::<CompiledClassHashes>(hash, compiled_hash)?;
            Ok(())
        })?
    }

    fn set_sierra_class(&self, hash: ClassHash, sierra: FlattenedSierraClass) -> Result<()> {
        self.0.update(move |db_tx| -> Result<()> {
            db_tx.put::<SierraClasses>(hash, sierra)?;
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
    fn class(&self, hash: ClassHash) -> Result<Option<CompiledContractClass>> {
        let class = self.0.get::<CompiledContractClasses>(hash)?;
        Ok(class.map(CompiledContractClass::from))
    }

    fn compiled_class_hash_of_class_hash(
        &self,
        hash: ClassHash,
    ) -> Result<Option<CompiledClassHash>> {
        let hash = self.0.get::<CompiledClassHashes>(hash)?;
        Ok(hash)
    }

    fn sierra_class(&self, hash: ClassHash) -> Result<Option<FlattenedSierraClass>> {
        let class = self.0.get::<SierraClasses>(hash)?;
        Ok(class)
    }
}

impl StateProvider for LatestStateProvider {
    fn nonce(&self, address: ContractAddress) -> Result<Option<Nonce>> {
        let info = self.0.get::<ContractInfo>(address)?;
        Ok(info.map(|info| info.nonce))
    }

    fn class_hash_of_contract(
        &self,
        address: ContractAddress,
    ) -> Result<Option<katana_primitives::contract::ClassHash>> {
        let info = self.0.get::<ContractInfo>(address)?;
        Ok(info.map(|info| info.class_hash))
    }

    fn storage(
        &self,
        address: ContractAddress,
        storage_key: StorageKey,
    ) -> Result<Option<StorageValue>> {
        let mut cursor = self.0.cursor::<ContractStorage>()?;
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
    ) -> Result<Option<CompiledClassHash>> {
        // check that the requested class hash was declared before the pinned block number
        if self.tx.get::<ClassDeclarationBlock>(hash)?.is_some_and(|num| num <= self.block_number) {
            Ok(self.tx.get::<CompiledClassHashes>(hash)?)
        } else {
            Ok(None)
        }
    }

    fn class(&self, hash: ClassHash) -> Result<Option<CompiledContractClass>> {
        if self.compiled_class_hash_of_class_hash(hash)?.is_some() {
            let contract = self.tx.get::<CompiledContractClasses>(hash)?;
            Ok(contract.map(CompiledContractClass::from))
        } else {
            Ok(None)
        }
    }

    fn sierra_class(&self, hash: ClassHash) -> Result<Option<FlattenedSierraClass>> {
        if self.compiled_class_hash_of_class_hash(hash)?.is_some() {
            self.tx.get::<SierraClasses>(hash).map_err(|e| e.into())
        } else {
            Ok(None)
        }
    }
}

impl StateProvider for HistoricalStateProvider {
    fn nonce(&self, address: ContractAddress) -> Result<Option<Nonce>> {
        let change_list = self.tx.get::<ContractInfoChangeSet>(address)?;

        if let Some(num) = change_list.and_then(|entry| {
            Self::recent_block_change_relative_to_pinned_block_num(
                self.block_number,
                &entry.nonce_change_list,
            )
        }) {
            let mut cursor = self.tx.cursor::<NonceChanges>()?;
            let ContractNonceChange { contract_address, nonce } = cursor
                .seek_by_key_subkey(num, address)?
                .expect("if block number is in the block set, change entry must exist");

            if contract_address == address {
                return Ok(Some(nonce));
            }
        }

        Ok(None)
    }

    fn class_hash_of_contract(&self, address: ContractAddress) -> Result<Option<ClassHash>> {
        let change_list: Option<ContractInfoChangeList> =
            self.tx.get::<ContractInfoChangeSet>(address)?;

        if let Some(num) = change_list.and_then(|entry| {
            Self::recent_block_change_relative_to_pinned_block_num(
                self.block_number,
                &entry.class_change_list,
            )
        }) {
            let mut cursor = self.tx.cursor::<ContractClassChanges>()?;
            let ContractClassChange { contract_address, class_hash } = cursor
                .seek_by_key_subkey(num, address)?
                .expect("if block number is in the block set, change entry must exist");

            if contract_address == address {
                return Ok(Some(class_hash));
            }
        }

        Ok(None)
    }

    fn storage(
        &self,
        address: ContractAddress,
        storage_key: StorageKey,
    ) -> Result<Option<StorageValue>> {
        let mut cursor = self.tx.cursor::<StorageChangeSet>()?;

        if let Some(num) = cursor.seek_by_key_subkey(address, storage_key)?.and_then(|entry| {
            Self::recent_block_change_relative_to_pinned_block_num(
                self.block_number,
                &entry.block_list,
            )
        }) {
            let mut cursor = self.tx.cursor::<StorageChanges>()?;
            let sharded_key = ContractStorageKey { contract_address: address, key: storage_key };

            let ContractStorageEntry { key, value } = cursor
                .seek_by_key_subkey(num, sharded_key)?
                .expect("if block number is in the block set, change entry must exist");

            if key.contract_address == address && key.key == storage_key {
                return Ok(Some(value));
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
