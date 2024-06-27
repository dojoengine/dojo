use core::fmt;
use std::collections::HashMap;
use std::str::FromStr;

use dojo_types::primitive::Primitive;
use dojo_types::schema::Ty;
use serde::{Deserialize, Serialize};
use starknet::core::types::{
    ContractStorageDiffItem, FromByteSliceError, FromStrError, StateDiff, StateUpdate, StorageEntry,
};
use starknet_crypto::FieldElement;
use strum_macros::{AsRefStr, EnumIter, FromRepr};

use crate::proto::{self};

pub mod schema;

#[derive(Debug, Serialize, Deserialize, PartialEq, Hash, Eq, Clone)]
pub struct Query {
    pub clause: Option<Clause>,
    pub limit: u32,
    pub offset: u32,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Hash, Eq, Clone)]
pub enum Clause {
    Keys(KeysClause),
    Member(MemberClause),
    Composite(CompositeClause),
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Hash, Eq, Clone)]
pub enum EntityKeysClause {
    HashedKeys(Vec<FieldElement>),
    Keys(KeysClause),
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Hash, Eq, Clone)]
pub struct ModelKeysClause {
    pub model: String,
    pub keys: Vec<FieldElement>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Hash, Eq, Clone)]
pub struct KeysClause {
    pub keys: Vec<FieldElement>,
    pub pattern_matching: PatternMatching,
    pub models: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Hash, Eq, Clone)]
pub enum PatternMatching {
    FixedLen,
    VariableLen,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Hash, Eq, Clone)]
pub struct MemberClause {
    pub model: String,
    pub member: String,
    pub operator: ComparisonOperator,
    pub value: Value,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Hash, Eq, Clone)]
pub struct CompositeClause {
    pub model: String,
    pub operator: LogicalOperator,
    pub clauses: Vec<Clause>,
}

#[derive(
    Debug, AsRefStr, Serialize, Deserialize, EnumIter, FromRepr, PartialEq, Hash, Eq, Clone,
)]
#[strum(serialize_all = "UPPERCASE")]
pub enum LogicalOperator {
    And,
    Or,
}

#[derive(
    Debug, AsRefStr, Serialize, Deserialize, EnumIter, FromRepr, PartialEq, Hash, Eq, Clone,
)]
#[strum(serialize_all = "UPPERCASE")]
pub enum ComparisonOperator {
    Eq,
    Neq,
    Gt,
    Gte,
    Lt,
    Lte,
}

impl fmt::Display for ComparisonOperator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ComparisonOperator::Gt => write!(f, ">"),
            ComparisonOperator::Gte => write!(f, ">="),
            ComparisonOperator::Lt => write!(f, "<"),
            ComparisonOperator::Lte => write!(f, "<="),
            ComparisonOperator::Neq => write!(f, "!="),
            ComparisonOperator::Eq => write!(f, "="),
        }
    }
}

impl From<proto::types::ComparisonOperator> for ComparisonOperator {
    fn from(operator: proto::types::ComparisonOperator) -> Self {
        match operator {
            proto::types::ComparisonOperator::Eq => ComparisonOperator::Eq,
            proto::types::ComparisonOperator::Gte => ComparisonOperator::Gte,
            proto::types::ComparisonOperator::Gt => ComparisonOperator::Gt,
            proto::types::ComparisonOperator::Lt => ComparisonOperator::Lt,
            proto::types::ComparisonOperator::Lte => ComparisonOperator::Lte,
            proto::types::ComparisonOperator::Neq => ComparisonOperator::Neq,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Hash, Eq, Clone)]
pub struct Value {
    pub primitive_type: Primitive,
    pub value_type: ValueType,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Hash, Eq, Clone)]
pub enum ValueType {
    String(String),
    Int(i64),
    UInt(u64),
    Bool(bool),
    Bytes(Vec<u8>),
}

impl TryFrom<proto::types::ModelMetadata> for dojo_types::schema::ModelMetadata {
    type Error = FromStrError;
    fn try_from(value: proto::types::ModelMetadata) -> Result<Self, Self::Error> {
        let schema: Ty = serde_json::from_slice(&value.schema).unwrap();
        let layout: Vec<FieldElement> = value.layout.into_iter().map(FieldElement::from).collect();
        Ok(Self {
            schema,
            layout,
            name: value.name,
            packed_size: value.packed_size,
            unpacked_size: value.unpacked_size,
            class_hash: FieldElement::from_str(&value.class_hash)?,
            contract_address: FieldElement::from_str(&value.contract_address)?,
        })
    }
}

impl TryFrom<proto::types::WorldMetadata> for dojo_types::WorldMetadata {
    type Error = FromStrError;
    fn try_from(value: proto::types::WorldMetadata) -> Result<Self, Self::Error> {
        let models = value
            .models
            .into_iter()
            .map(|component| Ok((component.name.clone(), component.try_into()?)))
            .collect::<Result<HashMap<_, dojo_types::schema::ModelMetadata>, _>>()?;

        Ok(dojo_types::WorldMetadata {
            models,
            world_address: FieldElement::from_str(&value.world_address)?,
            world_class_hash: FieldElement::from_str(&value.world_class_hash)?,
        })
    }
}

impl From<Query> for proto::types::Query {
    fn from(value: Query) -> Self {
        Self { clause: value.clause.map(|c| c.into()), limit: value.limit, offset: value.offset }
    }
}

impl From<proto::types::PatternMatching> for PatternMatching {
    fn from(value: proto::types::PatternMatching) -> Self {
        match value {
            proto::types::PatternMatching::FixedLen => PatternMatching::FixedLen,
            proto::types::PatternMatching::VariableLen => PatternMatching::VariableLen,
        }
    }
}

impl From<KeysClause> for proto::types::KeysClause {
    fn from(value: KeysClause) -> Self {
        Self {
            keys: value.keys.iter().map(|k| k.to_bytes_be().into()).collect(),
            pattern_matching: value.pattern_matching as i32,
            models: value.models,
        }
    }
}

impl TryFrom<proto::types::KeysClause> for KeysClause {
    type Error = FromByteSliceError;

    fn try_from(value: proto::types::KeysClause) -> Result<Self, Self::Error> {
        let keys = value
            .keys
            .iter()
            .map(|k| FieldElement::from_byte_slice_be(k))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self { keys, pattern_matching: value.pattern_matching().into(), models: value.models })
    }
}

impl From<Clause> for proto::types::Clause {
    fn from(value: Clause) -> Self {
        match value {
            Clause::Keys(clause) => {
                Self { clause_type: Some(proto::types::clause::ClauseType::Keys(clause.into())) }
            }
            Clause::Member(clause) => {
                Self { clause_type: Some(proto::types::clause::ClauseType::Member(clause.into())) }
            }
            Clause::Composite(clause) => Self {
                clause_type: Some(proto::types::clause::ClauseType::Composite(clause.into())),
            },
        }
    }
}

impl From<EntityKeysClause> for proto::types::EntityKeysClause {
    fn from(value: EntityKeysClause) -> Self {
        match value {
            EntityKeysClause::HashedKeys(hashed_keys) => Self {
                clause_type: Some(proto::types::entity_keys_clause::ClauseType::HashedKeys(
                    proto::types::HashedKeysClause {
                        hashed_keys: hashed_keys.iter().map(|k| k.to_bytes_be().into()).collect(),
                    },
                )),
            },
            EntityKeysClause::Keys(keys) => Self {
                clause_type: Some(proto::types::entity_keys_clause::ClauseType::Keys(keys.into())),
            },
        }
    }
}

impl TryFrom<proto::types::EntityKeysClause> for EntityKeysClause {
    type Error = FromByteSliceError;

    fn try_from(value: proto::types::EntityKeysClause) -> Result<Self, Self::Error> {
        match value.clause_type.expect("must have") {
            proto::types::entity_keys_clause::ClauseType::HashedKeys(clause) => {
                let keys = clause
                    .hashed_keys
                    .into_iter()
                    .map(|k| FieldElement::from_byte_slice_be(&k))
                    .collect::<Result<Vec<_>, _>>()?;

                Ok(Self::HashedKeys(keys))
            }
            proto::types::entity_keys_clause::ClauseType::Keys(clause) => {
                Ok(Self::Keys(clause.try_into()?))
            }
        }
    }
}

impl From<ModelKeysClause> for proto::types::ModelKeysClause {
    fn from(value: ModelKeysClause) -> Self {
        Self {
            model: value.model,
            keys: value.keys.iter().map(|k| k.to_bytes_be().into()).collect(),
        }
    }
}

impl TryFrom<proto::types::ModelKeysClause> for ModelKeysClause {
    type Error = FromByteSliceError;

    fn try_from(value: proto::types::ModelKeysClause) -> Result<Self, Self::Error> {
        let keys = value
            .keys
            .into_iter()
            .map(|k| FieldElement::from_byte_slice_be(&k))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self { model: value.model, keys })
    }
}

impl From<MemberClause> for proto::types::MemberClause {
    fn from(value: MemberClause) -> Self {
        Self {
            model: value.model,
            member: value.member,
            operator: value.operator as i32,
            value: Some(value.value.into()),
        }
    }
}

impl From<CompositeClause> for proto::types::CompositeClause {
    fn from(value: CompositeClause) -> Self {
        Self {
            model: value.model,
            operator: value.operator as i32,
            clauses: value.clauses.into_iter().map(|clause| clause.into()).collect(),
        }
    }
}

impl From<Value> for proto::types::Value {
    fn from(value: Value) -> Self {
        let value_type = match value.value_type {
            ValueType::String(val) => Some(proto::types::value::ValueType::StringValue(val)),
            ValueType::Int(val) => Some(proto::types::value::ValueType::IntValue(val)),
            ValueType::UInt(val) => Some(proto::types::value::ValueType::UintValue(val)),
            ValueType::Bool(val) => Some(proto::types::value::ValueType::BoolValue(val)),
            ValueType::Bytes(val) => Some(proto::types::value::ValueType::ByteValue(val)),
        };

        Self { value_type }
    }
}

impl TryFrom<proto::types::StorageEntry> for StorageEntry {
    type Error = FromStrError;
    fn try_from(value: proto::types::StorageEntry) -> Result<Self, Self::Error> {
        Ok(Self {
            key: FieldElement::from_str(&value.key)?,
            value: FieldElement::from_str(&value.value)?,
        })
    }
}

impl TryFrom<proto::types::StorageDiff> for ContractStorageDiffItem {
    type Error = FromStrError;
    fn try_from(value: proto::types::StorageDiff) -> Result<Self, Self::Error> {
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

impl TryFrom<proto::types::ModelDiff> for StateDiff {
    type Error = FromStrError;
    fn try_from(value: proto::types::ModelDiff) -> Result<Self, Self::Error> {
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

impl TryFrom<proto::types::ModelUpdate> for StateUpdate {
    type Error = FromStrError;
    fn try_from(value: proto::types::ModelUpdate) -> Result<Self, Self::Error> {
        Ok(Self {
            new_root: FieldElement::ZERO,
            old_root: FieldElement::ZERO,
            block_hash: FieldElement::from_str(&value.block_hash)?,
            state_diff: value.model_diff.expect("must have").try_into()?,
        })
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Hash, Eq, Clone)]
pub struct Event {
    pub keys: Vec<FieldElement>,
    pub data: Vec<FieldElement>,
    pub transaction_hash: FieldElement,
}

impl TryFrom<proto::types::Event> for Event {
    type Error = FromByteSliceError;

    fn try_from(value: proto::types::Event) -> Result<Self, Self::Error> {
        let keys = value
            .keys
            .into_iter()
            .map(|k| FieldElement::from_byte_slice_be(&k))
            .collect::<Result<Vec<_>, _>>()?;

        let data = value
            .data
            .into_iter()
            .map(|d| FieldElement::from_byte_slice_be(&d))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self {
            keys,
            data,
            transaction_hash: FieldElement::from_byte_slice_be(&value.transaction_hash)?,
        })
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Hash, Eq, Clone)]
pub struct EventQuery {
    pub keys: KeysClause,
    pub limit: u32,
    pub offset: u32,
}

impl From<EventQuery> for proto::types::EventQuery {
    fn from(value: EventQuery) -> Self {
        Self { keys: Some(value.keys.into()), limit: value.limit, offset: value.offset }
    }
}
