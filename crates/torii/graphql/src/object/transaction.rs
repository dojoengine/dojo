use super::{ObjectTrait, TypeMapping};
use crate::mapping::TRANSACTION_MAPPING;
use crate::query::constants::TRANSACTION_TABLE;

pub struct TransactionObject;

impl ObjectTrait for TransactionObject {
    fn name(&self) -> (&str, &str) {
        ("transaction", "transactions")
    }

    fn type_name(&self) -> &str {
        "Transaction"
    }

    fn type_mapping(&self) -> &TypeMapping {
        &TRANSACTION_MAPPING
    }

    fn table_name(&self) -> Option<&str> {
        Some(TRANSACTION_TABLE)
    }
}
