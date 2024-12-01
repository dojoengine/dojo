use std::collections::{BTreeMap, BTreeSet};

pub use katana_primitives::class::CasmContractClass;
use katana_primitives::class::{
    ClassHash, CompiledClassHash, LegacyContractClass, SierraContractClass,
};
use katana_primitives::contract::{Nonce, StorageKey, StorageValue};
use katana_primitives::transaction::TxHash;
use katana_primitives::{ContractAddress, Felt};
use katana_rpc_types::class::ConversionError;
pub use katana_rpc_types::class::RpcSierraContractClass;
use serde::Deserialize;
use starknet::providers::sequencer::models::Block;

/// The contract class type returns by `/get_class_by_hash` endpoint.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum ContractClass {
    Class(RpcSierraContractClass),
    Legacy(LegacyContractClass),
}

/// The state update type returns by `/get_state_update` endpoint.
#[derive(Debug, Deserialize)]
pub struct StateUpdate {
    pub block_hash: Option<Felt>,
    pub new_root: Option<Felt>,
    pub old_root: Felt,
    pub state_diff: StateDiff,
}

#[derive(Debug, Deserialize)]
pub struct StateDiff {
    pub storage_diffs: BTreeMap<ContractAddress, Vec<StorageDiff>>,
    pub deployed_contracts: Vec<DeployedContract>,
    pub old_declared_contracts: Vec<Felt>,
    pub declared_classes: Vec<DeclaredContract>,
    pub nonces: BTreeMap<ContractAddress, Nonce>,
    pub replaced_classes: Vec<DeployedContract>,
}

#[derive(Debug, Deserialize)]
pub struct StorageDiff {
    pub key: StorageKey,
    pub value: StorageValue,
}

#[derive(Debug, Deserialize)]
pub struct DeployedContract {
    pub address: ContractAddress,
    pub class_hash: Felt,
}

#[derive(Debug, Deserialize)]
pub struct DeclaredContract {
    pub class_hash: ClassHash,
    pub compiled_class_hash: CompiledClassHash,
}

/// The state update type returns by `/get_state_update` endpoint, with `includeBlock=true`.
#[derive(Debug, Deserialize)]
pub struct StateUpdateWithBlock {
    pub state_update: StateUpdate,
    pub block: Block,
}

pub struct TxWithHash {
    pub hash: TxHash,
    pub tx: Tx,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum Tx {
    #[serde(rename = "DECLARE")]
    Declare,
    #[serde(rename = "DECLARE")]
    Deploy,
    #[serde(rename = "DEPLOY_ACCOUNT")]
    DeployAccount(DeployAccountTx),
    #[serde(rename = "INVOKE_FUNCTION")]
    Invoke(InvokeTx),
    #[serde(rename = "L1_HANDLER")]
    L1Handler,
}

#[derive(Debug)]
pub enum DeclareTx {
    V1,
    V2,
    V3,
}

#[derive(Debug)]
pub enum DeployAccountTx {
    V1,
    V3,
}

#[derive(Debug)]
pub enum InvokeTx {
    V0,
    V1,
    V3,
}

impl<'de> Deserialize<'de> for TxWithHash {
    fn deserialize<D>(deserializer: D) -> Result<TxWithHash, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Helper {
            #[serde(flatten)]
            tx: Tx,
            #[serde(rename = "transaction_hash")]
            hash: TxHash,
        }

        let Helper { hash, tx } = Helper::deserialize(deserializer)?;
        Ok(Self { hash, tx })
    }
}

impl<'de> Deserialize<'de> for DeclareTx {
    fn deserialize<D>(deserializer: D) -> Result<DeclareTx, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Helper {
            version: u8,
        }

        let h = Helper::deserialize(deserializer)?;
        match h.version {
            1 => Ok(DeclareTx::V1),
            2 => Ok(DeclareTx::V2),
            3 => Ok(DeclareTx::V3),
            v => Err(serde::de::Error::custom(format!("unknown version: {}", v))),
        }
    }
}

impl<'de> Deserialize<'de> for DeployAccountTx {
    fn deserialize<D>(deserializer: D) -> Result<DeployAccountTx, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Helper {
            version: Felt,
        }

        let Helper { version } = Helper::deserialize(deserializer)?;

        if version == Felt::ONE {
            Ok(DeployAccountTx::V1)
        } else if version == Felt::THREE {
            Ok(DeployAccountTx::V3)
        } else {
            Err(serde::de::Error::custom(format!("unknown version: {version}")))
        }
    }
}

impl<'de> Deserialize<'de> for InvokeTx {
    fn deserialize<D>(deserializer: D) -> Result<InvokeTx, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Helper {
            version: Felt,
        }

        let Helper { version } = Helper::deserialize(deserializer)?;

        if version == Felt::ZERO {
            Ok(InvokeTx::V0)
        } else if version == Felt::ONE {
            Ok(InvokeTx::V1)
        } else if version == Felt::THREE {
            Ok(InvokeTx::V3)
        } else {
            Err(serde::de::Error::custom(format!("unknown version: {version}")))
        }
    }
}

// -- Conversion to Katana primitive types.

impl TryFrom<ContractClass> for katana_primitives::class::ContractClass {
    type Error = ConversionError;

    fn try_from(value: ContractClass) -> Result<Self, Self::Error> {
        match value {
            ContractClass::Legacy(class) => Ok(Self::Legacy(class)),
            ContractClass::Class(class) => {
                let class = SierraContractClass::try_from(class)?;
                Ok(Self::Class(class))
            }
        }
    }
}

impl From<StateDiff> for katana_primitives::state::StateUpdates {
    fn from(value: StateDiff) -> Self {
        let storage_updates = value
            .storage_diffs
            .into_iter()
            .map(|(addr, diffs)| {
                let storage_map = diffs.into_iter().map(|diff| (diff.key, diff.value)).collect();
                (addr, storage_map)
            })
            .collect();

        let deployed_contracts = value
            .deployed_contracts
            .into_iter()
            .map(|contract| (contract.address, contract.class_hash))
            .collect();

        let declared_classes = value
            .declared_classes
            .into_iter()
            .map(|contract| (contract.class_hash, contract.compiled_class_hash))
            .collect();

        let replaced_classes = value
            .replaced_classes
            .into_iter()
            .map(|contract| (contract.address, contract.class_hash))
            .collect();

        Self {
            storage_updates,
            declared_classes,
            replaced_classes,
            deployed_contracts,
            nonce_updates: value.nonces,
            deprecated_declared_classes: BTreeSet::from_iter(value.old_declared_contracts),
        }
    }
}

#[cfg(test)]
mod tests {
    use katana_primitives::felt;
    use serde_json;

    use super::*;

    #[test]
    fn test_tx_with_hash_deserialization() {
        let json = r#"{
            "type": "INVOKE_FUNCTION",
            "transaction_hash": "0x123",
            "version": "0x0"
        }"#;

        let tx: TxWithHash = serde_json::from_str(json).unwrap();

        assert!(matches!(tx.tx, Tx::Invoke(InvokeTx::V0)));
        assert_eq!(tx.hash, felt!("0x123"));

        let json = r#"{
            "type": "INVOKE_FUNCTION",
            "transaction_hash": "0x123",
            "version": "0x1"
        }"#;

        let tx: TxWithHash = serde_json::from_str(json).unwrap();

        assert!(matches!(tx.tx, Tx::Invoke(InvokeTx::V1)));
        assert_eq!(tx.hash, felt!("0x123"));

        let json = r#"{
            "type": "DEPLOY_ACCOUNT",
            "transaction_hash": "0x123",
            "version": "0x3"
        }"#;

        let tx: TxWithHash = serde_json::from_str(json).unwrap();

        assert!(matches!(tx.tx, Tx::DeployAccount(DeployAccountTx::V3)));
        assert_eq!(tx.hash, felt!("0x123"));
    }
}
