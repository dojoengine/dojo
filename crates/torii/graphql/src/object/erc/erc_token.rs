use crate::constants::{ERC_TOKEN_NAME, ERC_TOKEN_TYPE_NAME};
use crate::mapping::ERC_TOKEN_TYPE_MAPPING;
use crate::object::BasicObject;
use crate::types::TypeMapping;

#[derive(Debug)]
pub struct ErcTokenObject;

impl BasicObject for ErcTokenObject {
    fn name(&self) -> (&str, &str) {
        ERC_TOKEN_NAME
    }

    fn type_name(&self) -> &str {
        ERC_TOKEN_TYPE_NAME
    }

    fn type_mapping(&self) -> &TypeMapping {
        &ERC_TOKEN_TYPE_MAPPING
    }
}
