use super::{BasicObjectTrait, ResolvableObjectTrait, TypeMapping};
use crate::constants::{TRANSACTION_NAMES, TRANSACTION_TABLE, TRANSACTION_TYPE_NAME};
use crate::mapping::TRANSACTION_MAPPING;

pub struct TransactionObject;

impl BasicObjectTrait for TransactionObject {
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

impl ResolvableObjectTrait for TransactionObject {
    fn table_name(&self) -> &str {
        TRANSACTION_TABLE
    }
}
