use super::{ObjectTrait, TypeMapping};
use crate::constants::{TRANSACTION_NAMES, TRANSACTION_TABLE, TRANSACTION_TYPE_NAME};
use crate::mapping::TRANSACTION_MAPPING;

pub struct TransactionObject;

impl ObjectTrait for TransactionObject {
    fn name(&self) -> (&str, &str) {
        TRANSACTION_NAMES
    }

    fn type_name(&self) -> &str {
        TRANSACTION_TYPE_NAME
    }

    fn type_mapping(&self) -> &TypeMapping {
        &TRANSACTION_MAPPING
    }

    fn table_name(&self) -> Option<&str> {
        Some(TRANSACTION_TABLE)
    }
}
