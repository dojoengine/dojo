use core::fmt;
use std::collections::HashMap;
use std::str::FromStr;

use crypto_bigint::U256;
use dojo_types::primitive::Primitive;
use dojo_types::schema::Ty;
use dojo_world::contracts::naming;
use schema::SchemaError;
use serde::{Deserialize, Serialize};
use starknet::core::types::{
    ContractStorageDiffItem, Felt, FromStrError, StateDiff, StateUpdate, StorageEntry,
};
use strum_macros::{AsRefStr, EnumIter, FromRepr};

use crate::proto::types::member_value;
use crate::proto::{self};

pub mod schema;

#[derive(Debug, Serialize, Deserialize, PartialEq, Hash, Eq, Clone)]
pub struct Token {
    pub contract_address: Felt,
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
    pub metadata: String,
}

impl TryFrom<proto::types::Token> for Token {
    type Error = SchemaError;
    fn try_from(value: proto::types::Token) -> Result<Self, Self::Error> {
        Ok(Self {
            contract_address: Felt::from_str(&value.contract_address)?,
            name: value.name,
            symbol: value.symbol,
            decimals: value.decimals as u8,
            metadata: value.metadata,
        })
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Hash, Eq, Clone)]
pub struct TokenBalance {
    pub balance: U256,
    pub account_address: Felt,
    pub contract_address: Felt,
    pub token_id: String,
}

impl TryFrom<proto::types::TokenBalance> for TokenBalance {
    type Error = SchemaError;
    fn try_from(value: proto::types::TokenBalance) -> Result<Self, Self::Error> {
        Ok(Self {
            balance: U256::from_be_hex(&value.balance),
            account_address: Felt::from_str(&value.account_address)?,
            contract_address: Felt::from_str(&value.contract_address)?,
            token_id: value.token_id,
        })
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Hash, Eq, Clone)]
pub struct IndexerUpdate {
    pub head: i64,
    pub tps: i64,
    pub last_block_timestamp: i64,
    pub contract_address: Felt,
}

impl From<proto::world::SubscribeIndexerResponse> for IndexerUpdate {
    fn from(value: proto::world::SubscribeIndexerResponse) -> Self {
        Self {
            head: value.head,
            tps: value.tps,
            last_block_timestamp: value.last_block_timestamp,
            contract_address: Felt::from_bytes_be_slice(&value.contract_address),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Hash, Eq, Clone)]
pub struct Query {
    pub clause: Option<Clause>,
    pub limit: u32,
    pub offset: u32,
    pub dont_include_hashed_keys: bool,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Hash, Eq, Clone)]
pub enum Clause {
    Keys(KeysClause),
    Member(MemberClause),
    Composite(CompositeClause),
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Hash, Eq, Clone)]
pub enum EntityKeysClause {
    HashedKeys(Vec<Felt>),
    Keys(KeysClause),
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Hash, Eq, Clone)]
pub struct ModelKeysClause {
    pub model: String,
    pub keys: Vec<Felt>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Hash, Eq, Clone)]
pub struct KeysClause {
    pub keys: Vec<Option<Felt>>,
    pub pattern_matching: PatternMatching,
    pub models: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Hash, Eq, Clone)]
pub enum PatternMatching {
    FixedLen,
    VariableLen,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Hash, Eq, Clone)]
pub enum MemberValue {
    Primitive(Primitive),
    String(String),
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Hash, Eq, Clone)]
pub struct MemberClause {
    pub model: String,
    pub member: String,
    pub operator: ComparisonOperator,
    pub value: MemberValue,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Hash, Eq, Clone)]
pub struct CompositeClause {
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
        let layout: Vec<Felt> = value.layout.into_iter().map(Felt::from).collect();
        Ok(Self {
            schema,
            layout,
            name: value.name,
            namespace: value.namespace,
            packed_size: value.packed_size,
            unpacked_size: value.unpacked_size,
            class_hash: Felt::from_str(&value.class_hash)?,
            contract_address: Felt::from_str(&value.contract_address)?,
        })
    }
}

impl TryFrom<proto::types::WorldMetadata> for dojo_types::WorldMetadata {
    type Error = FromStrError;
    fn try_from(value: proto::types::WorldMetadata) -> Result<Self, Self::Error> {
        let models = value
            .models
            .into_iter()
            .map(|component| {
                Ok((
                    naming::compute_selector_from_names(&component.namespace, &component.name),
                    component.try_into()?,
                ))
            })
            .collect::<Result<HashMap<_, dojo_types::schema::ModelMetadata>, _>>()?;

        Ok(dojo_types::WorldMetadata {
            models,
            world_address: Felt::from_str(&value.world_address)?,
        })
    }
}

impl From<Query> for proto::types::Query {
    fn from(value: Query) -> Self {
        Self {
            clause: value.clause.map(|c| c.into()),
            limit: value.limit,
            offset: value.offset,
            dont_include_hashed_keys: value.dont_include_hashed_keys,
        }
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
            keys: value
                .keys
                .iter()
                .map(|k| k.map_or(Vec::new(), |k| k.to_bytes_be().into()))
                .collect(),
            pattern_matching: value.pattern_matching as i32,
            models: value.models,
        }
    }
}

impl From<proto::types::KeysClause> for KeysClause {
    fn from(value: proto::types::KeysClause) -> Self {
        let keys = value
            .keys
            .iter()
            .map(|k| if k.is_empty() { None } else { Some(Felt::from_bytes_be_slice(k)) })
            .collect::<Vec<Option<Felt>>>();

        Self { keys, pattern_matching: value.pattern_matching().into(), models: value.models }
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

impl From<proto::types::EntityKeysClause> for EntityKeysClause {
    fn from(value: proto::types::EntityKeysClause) -> Self {
        match value.clause_type.expect("must have") {
            proto::types::entity_keys_clause::ClauseType::HashedKeys(clause) => {
                let keys = clause
                    .hashed_keys
                    .into_iter()
                    .map(|k| Felt::from_bytes_be_slice(&k))
                    .collect::<Vec<_>>();

                Self::HashedKeys(keys)
            }

            proto::types::entity_keys_clause::ClauseType::Keys(clause) => Self::Keys(clause.into()),
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

impl From<proto::types::ModelKeysClause> for ModelKeysClause {
    fn from(value: proto::types::ModelKeysClause) -> Self {
        let keys = value.keys.into_iter().map(|v| Felt::from_bytes_be_slice(&v)).collect();
        Self { model: value.model, keys }
    }
}

impl From<MemberClause> for proto::types::MemberClause {
    fn from(value: MemberClause) -> Self {
        Self {
            model: value.model,
            member: value.member,
            operator: value.operator as i32,
            value: Some(proto::types::MemberValue { value_type: Some(value.value.into()) }),
        }
    }
}

impl From<CompositeClause> for proto::types::CompositeClause {
    fn from(value: CompositeClause) -> Self {
        Self {
            operator: value.operator as i32,
            clauses: value.clauses.into_iter().map(|clause| clause.into()).collect(),
        }
    }
}

impl From<MemberValue> for member_value::ValueType {
    fn from(value: MemberValue) -> Self {
        match value {
            MemberValue::Primitive(primitive) => {
                member_value::ValueType::Primitive(primitive.into())
            }
            MemberValue::String(string) => member_value::ValueType::String(string),
        }
    }
}

impl TryFrom<proto::types::StorageEntry> for StorageEntry {
    type Error = FromStrError;
    fn try_from(value: proto::types::StorageEntry) -> Result<Self, Self::Error> {
        Ok(Self { key: Felt::from_str(&value.key)?, value: Felt::from_str(&value.value)? })
    }
}

impl TryFrom<proto::types::StorageDiff> for ContractStorageDiffItem {
    type Error = FromStrError;
    fn try_from(value: proto::types::StorageDiff) -> Result<Self, Self::Error> {
        Ok(Self {
            address: Felt::from_str(&value.address)?,
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
            new_root: Felt::ZERO,
            old_root: Felt::ZERO,
            block_hash: Felt::from_str(&value.block_hash)?,
            state_diff: value.model_diff.expect("must have").try_into()?,
        })
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Hash, Eq, Clone)]
pub struct Event {
    pub keys: Vec<Felt>,
    pub data: Vec<Felt>,
    pub transaction_hash: Felt,
}

impl From<proto::types::Event> for Event {
    fn from(value: proto::types::Event) -> Self {
        let keys = value.keys.into_iter().map(|k| Felt::from_bytes_be_slice(&k)).collect();
        let data = value.data.into_iter().map(|d| Felt::from_bytes_be_slice(&d)).collect();
        let transaction_hash = Felt::from_bytes_be_slice(&value.transaction_hash);
        Self { keys, data, transaction_hash }
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
