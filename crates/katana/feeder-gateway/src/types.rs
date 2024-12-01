use std::collections::{BTreeMap, BTreeSet};

pub use katana_primitives::class::CasmContractClass;
use katana_primitives::class::{
    ClassHash, CompiledClassHash, LegacyContractClass, SierraContractClass,
};
use katana_primitives::contract::{Nonce, StorageKey, StorageValue};
use katana_primitives::transaction::{InvokeTx, TxHash};
use katana_primitives::{ContractAddress, Felt};
use katana_rpc_types::class::ConversionError;
pub use katana_rpc_types::class::RpcSierraContractClass;
use serde::{Deserialize, Deserializer};
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

#[derive(Debug, Deserialize)]
pub struct Transaction {
    #[serde(rename = "transaction_hash")]
    pub hash: TxHash,
    #[serde(flatten)]
    pub tx: TypedTransaction,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TypedTransaction {
    Declare,
    Deploy,
    DeployAccount(DeployAccountTx),
    #[serde(deserialize_with = "deserialize_invoke")]
    InvokeFunction(InvokeTx),
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

fn deserialize_invoke<'de, D>(deserializer: D) -> Result<InvokeTx, D::Error>
where
    D: Deserializer<'de>,
{
    use katana_primitives::transaction::{InvokeTxV0, InvokeTxV1};

    #[derive(Debug, Deserialize)]
    struct Helper {
        version: TxHash,
        #[serde(flatten)]
        value: serde_json::Value,
    }

    let Helper { version, value } = Helper::deserialize(deserializer)?;

    if version == Felt::ZERO {
        let tx = serde_json::from_value::<InvokeTxV0>(value).map_err(serde::de::Error::custom)?;
        Ok(InvokeTx::V0(tx))
    } else if version == Felt::ONE {
        let tx = serde_json::from_value::<InvokeTxV1>(value).map_err(serde::de::Error::custom)?;
        Ok(InvokeTx::V1(tx))
    } else if version == Felt::THREE {
        Ok(InvokeTx::V1(Default::default()))
    } else {
        Err(serde::de::Error::custom(format!("unknown version: {version}")))
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
            "sender_address": "0x456",
            "nonce": "0x1",
            "entry_point_selector": "0x1",
            "calldata": [],
            "signature": [],
            "version": "0x0"
        }"#;

        let tx: Transaction = serde_json::from_str(json).unwrap();

        assert!(matches!(tx.tx, TypedTransaction::Invoke(InvokeTx::V0(..))));
        assert_eq!(tx.hash, felt!("0x123"));

        if let TypedTransaction::Invoke(InvokeTx::V0(v0)) = tx.tx {
            assert_eq!(v0.sender_address, felt!("0x456").into());
            assert_eq!(v0.nonce, felt!("0x1").into());
            assert_eq!(v0.entry_point_selector, felt!("0x1"));
            assert_eq!(v0.calldata.len(), 0);
            assert_eq!(v0.signature.len(), 0);
        } else {
            panic!("wrong variant")
        }
    }
}
