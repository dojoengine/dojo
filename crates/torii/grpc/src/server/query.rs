use serde::{Deserialize, Serialize};
use starknet_crypto::FieldElement;

#[derive(Debug, Serialize, Deserialize, PartialEq, Hash, Eq, Clone)]
pub struct Query {
    pub model: String,
    pub clause: Clause,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Hash, Eq, Clone)]
pub enum Clause {
    Keys(KeysClause),
    Attribute(AttributeClause),
    Composite(CompositeClause),
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Hash, Eq, Clone)]
pub struct KeysClause {
    pub keys: Vec<FieldElement>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Hash, Eq, Clone)]
pub struct AttributeClause {
    pub attribute: String,
    pub operator: ComparisonOperator,
    pub value: Value,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Hash, Eq, Clone)]
pub struct CompositeClause {
    pub operator: LogicalOperator,
    pub clauses: Vec<Clause>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Hash, Eq, Clone)]
pub enum LogicalOperator {
    And,
    Or,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Hash, Eq, Clone)]
pub enum ComparisonOperator {
    Eq,
    Neq,
    Gt,
    Gte,
    Lt,
    Lte,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Hash, Eq, Clone)]
pub enum Value {
    String(String),
    Int(i64),
    UInt(u64),
    Bool(bool),
    Bytes(Vec<u8>),
}
