use super::{BasicObject, TypeMapping};
use crate::constants::{EMPTY_NAMES, EMPTY_TYPE_NAME};
use crate::mapping::EMPTY_MAPPING;

#[derive(Debug)]
pub struct EmptyObject;

impl BasicObject for EmptyObject {
    fn name(&self) -> (&str, &str) {
        EMPTY_NAMES
    }

    fn type_name(&self) -> &str {
        EMPTY_TYPE_NAME
    }

    fn type_mapping(&self) -> &TypeMapping {
        &EMPTY_MAPPING
    }
} 