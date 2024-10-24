use crate::constants::{TOKEN_NAME, TOKEN_TYPE_NAME};
use crate::mapping::TOKEN_TYPE_MAPPING;
use crate::object::BasicObject;
use crate::types::TypeMapping;

#[derive(Debug)]
pub struct ErcTokenObject;

impl BasicObject for ErcTokenObject {
    fn name(&self) -> (&str, &str) {
        TOKEN_NAME
    }

    fn type_name(&self) -> &str {
        TOKEN_TYPE_NAME
    }

    fn type_mapping(&self) -> &TypeMapping {
        &TOKEN_TYPE_MAPPING
    }
}
