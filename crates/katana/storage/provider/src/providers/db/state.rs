use anyhow::Result;
use katana_db::mdbx::{self, DbEnv};
use katana_db::models::storage::StorageEntry;
use katana_db::tables::{
    CompiledClassHashes, CompiledContractClasses, ContractInfo, ContractStorage, SierraClasses,
};
use katana_primitives::contract::{
    ClassHash, CompiledClassHash, CompiledContractClass, ContractAddress, GenericContractInfo,
    Nonce, SierraClass, StorageKey, StorageValue,
};

use crate::traits::contract::{ContractClassProvider, ContractClassWriter};
use crate::traits::state::{StateProvider, StateWriter};

impl StateWriter for DbEnv {
    fn set_nonce(&self, address: ContractAddress, nonce: Nonce) -> Result<()> {
        self.update(move |db_tx| -> Result<()> {
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
        self.update(move |db_tx| -> Result<()> {
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
        self.update(move |db_tx| -> Result<()> {
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

impl ContractClassWriter for DbEnv {
    fn set_class(&self, hash: ClassHash, class: CompiledContractClass) -> Result<()> {
        self.update(move |db_tx| -> Result<()> {
            db_tx.put::<CompiledContractClasses>(hash, class.into())?;
            Ok(())
        })?
    }

    fn set_compiled_class_hash_of_class_hash(
        &self,
        hash: ClassHash,
        compiled_hash: CompiledClassHash,
    ) -> Result<()> {
        self.update(move |db_tx| -> Result<()> {
            db_tx.put::<CompiledClassHashes>(hash, compiled_hash)?;
            Ok(())
        })?
    }

    fn set_sierra_class(&self, hash: ClassHash, sierra: SierraClass) -> Result<()> {
        self.update(move |db_tx| -> Result<()> {
            db_tx.put::<SierraClasses>(hash, sierra)?;
            Ok(())
        })?
    }
}

pub struct LatestStateProvider(pub(super) mdbx::tx::TxRO);

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

    fn sierra_class(&self, hash: ClassHash) -> Result<Option<SierraClass>> {
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
