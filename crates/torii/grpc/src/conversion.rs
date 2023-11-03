use std::collections::HashMap;
use std::str::FromStr;

use dojo_types::schema::{
    AttributeClause, Clause, CompositeClause, EntityQuery, KeysClause, Ty, Value,
};
use starknet::core::types::{
    ContractStorageDiffItem, FromByteSliceError, FromStrError, StateDiff, StateUpdate, StorageEntry,
};
use starknet_crypto::FieldElement;

use crate::protos;

impl TryFrom<protos::types::ModelMetadata> for dojo_types::schema::ModelMetadata {
    type Error = FromStrError;
    fn try_from(value: protos::types::ModelMetadata) -> Result<Self, Self::Error> {
        let schema: Ty = serde_json::from_slice(&value.schema).unwrap();
        let layout: Vec<FieldElement> = value.layout.into_iter().map(FieldElement::from).collect();
        Ok(Self {
            schema,
            layout,
            name: value.name,
            packed_size: value.packed_size,
            unpacked_size: value.unpacked_size,
            class_hash: FieldElement::from_str(&value.class_hash)?,
        })
    }
}

impl TryFrom<protos::types::WorldMetadata> for dojo_types::WorldMetadata {
    type Error = FromStrError;
    fn try_from(value: protos::types::WorldMetadata) -> Result<Self, Self::Error> {
        let models = value
            .models
            .into_iter()
            .map(|component| Ok((component.name.clone(), component.try_into()?)))
            .collect::<Result<HashMap<_, dojo_types::schema::ModelMetadata>, _>>()?;

        Ok(dojo_types::WorldMetadata {
            models,
            world_address: FieldElement::from_str(&value.world_address)?,
            world_class_hash: FieldElement::from_str(&value.world_class_hash)?,
            executor_address: FieldElement::from_str(&value.executor_address)?,
            executor_class_hash: FieldElement::from_str(&value.executor_class_hash)?,
        })
    }
}

impl From<EntityQuery> for protos::types::EntityQuery {
    fn from(value: EntityQuery) -> Self {
        Self { model: value.model, clause: Some(value.clause.into()) }
    }
}

impl From<Clause> for protos::types::Clause {
    fn from(value: Clause) -> Self {
        match value {
            Clause::Keys(clause) => {
                Self { clause_type: Some(protos::types::clause::ClauseType::Keys(clause.into())) }
            }
            Clause::Attribute(clause) => Self {
                clause_type: Some(protos::types::clause::ClauseType::Attribute(clause.into())),
            },
            Clause::Composite(clause) => Self {
                clause_type: Some(protos::types::clause::ClauseType::Composite(clause.into())),
            },
        }
    }
}

impl From<KeysClause> for protos::types::KeysClause {
    fn from(value: KeysClause) -> Self {
        Self { keys: value.keys.iter().map(|k| k.to_bytes_be().into()).collect() }
    }
}

impl TryFrom<protos::types::KeysClause> for KeysClause {
    type Error = FromByteSliceError;

    fn try_from(value: protos::types::KeysClause) -> Result<Self, Self::Error> {
        let keys = value
            .keys
            .into_iter()
            .map(|k| FieldElement::from_byte_slice_be(&k))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self { keys })
    }
}

impl From<AttributeClause> for protos::types::AttributeClause {
    fn from(value: AttributeClause) -> Self {
        Self {
            attribute: value.attribute,
            operator: value.operator as i32,
            value: Some(value.value.into()),
        }
    }
}

impl From<CompositeClause> for protos::types::CompositeClause {
    fn from(value: CompositeClause) -> Self {
        Self {
            operator: value.operator as i32,
            clauses: value.clauses.into_iter().map(|clause| clause.into()).collect(),
        }
    }
}

impl From<Value> for protos::types::Value {
    fn from(value: Value) -> Self {
        match value {
            Value::String(val) => {
                Self { value_type: Some(protos::types::value::ValueType::StringValue(val)) }
            }
            Value::Int(val) => {
                Self { value_type: Some(protos::types::value::ValueType::IntValue(val)) }
            }
            Value::UInt(val) => {
                Self { value_type: Some(protos::types::value::ValueType::UintValue(val)) }
            }
            Value::Bool(val) => {
                Self { value_type: Some(protos::types::value::ValueType::BoolValue(val)) }
            }
            Value::Bytes(val) => {
                Self { value_type: Some(protos::types::value::ValueType::ByteValue(val)) }
            }
        }
    }
}

impl TryFrom<protos::types::StorageEntry> for StorageEntry {
    type Error = FromStrError;
    fn try_from(value: protos::types::StorageEntry) -> Result<Self, Self::Error> {
        Ok(Self {
            key: FieldElement::from_str(&value.key)?,
            value: FieldElement::from_str(&value.value)?,
        })
    }
}

impl TryFrom<protos::types::StorageDiff> for ContractStorageDiffItem {
    type Error = FromStrError;
    fn try_from(value: protos::types::StorageDiff) -> Result<Self, Self::Error> {
        Ok(Self {
            address: FieldElement::from_str(&value.address)?,
            storage_entries: value
                .storage_entries
                .into_iter()
                .map(|entry| entry.try_into())
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}

impl TryFrom<protos::types::EntityDiff> for StateDiff {
    type Error = FromStrError;
    fn try_from(value: protos::types::EntityDiff) -> Result<Self, Self::Error> {
        Ok(Self {
            nonces: vec![],
            declared_classes: vec![],
            replaced_classes: vec![],
            deployed_contracts: vec![],
            deprecated_declared_classes: vec![],
            storage_diffs: value
                .storage_diffs
                .into_iter()
                .map(|diff| diff.try_into())
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}

impl TryFrom<protos::types::EntityUpdate> for StateUpdate {
    type Error = FromStrError;
    fn try_from(value: protos::types::EntityUpdate) -> Result<Self, Self::Error> {
        Ok(Self {
            new_root: FieldElement::ZERO,
            old_root: FieldElement::ZERO,
            block_hash: FieldElement::from_str(&value.block_hash)?,
            state_diff: value.entity_diff.expect("must have").try_into()?,
        })
    }
}
