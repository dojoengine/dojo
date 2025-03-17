use async_graphql::dynamic::Field;

use super::{BasicObject, ResolvableObject, TypeMapping};
use crate::constants::{
    FUNCTION_CALL_TYPE_NAME, ID_COLUMN, TRANSACTION_HASH_COLUMN, TRANSACTION_NAMES,
    TRANSACTION_TABLE, TRANSACTION_TYPE_NAME,
};
use crate::mapping::{FUNCTION_CALL_MAPPING, TRANSACTION_MAPPING};
use crate::object::{resolve_many, resolve_one};

#[derive(Debug)]
pub struct FunctionCallObject;

impl BasicObject for FunctionCallObject {
    fn name(&self) -> (&str, &str) {
        ("functionCall", "")
    }

    fn type_name(&self) -> &str {
        FUNCTION_CALL_TYPE_NAME
    }

    fn type_mapping(&self) -> &TypeMapping {
        &FUNCTION_CALL_MAPPING
    }
}

#[derive(Debug)]
pub struct TransactionObject;

impl BasicObject for TransactionObject {
    fn name(&self) -> (&str, &str) {
        TRANSACTION_NAMES
    }

    fn type_name(&self) -> &str {
        TRANSACTION_TYPE_NAME
    }

    fn type_mapping(&self) -> &TypeMapping {
        &TRANSACTION_MAPPING
    }
}

impl ResolvableObject for TransactionObject {
    fn resolvers(&self) -> Vec<Field> {
        let resolve_one = resolve_one(
            TRANSACTION_TABLE,
            TRANSACTION_HASH_COLUMN,
            self.name().0,
            self.type_name(),
            self.type_mapping(),
        );

        let resolve_many = resolve_many(
            TRANSACTION_TABLE,
            ID_COLUMN,
            self.name().1,
            self.type_name(),
            self.type_mapping(),
        );

        vec![resolve_one, resolve_many]
    }
}
