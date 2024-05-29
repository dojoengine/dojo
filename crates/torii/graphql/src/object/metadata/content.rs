use super::TypeMapping;
use crate::constants::{CONTENT_NAMES, CONTENT_TYPE_NAME};
use crate::mapping::CONTENT_TYPE_MAPPING;
use crate::object::BasicObject;

#[derive(Debug)]
pub struct ContentObject;

impl BasicObject for ContentObject {
    fn name(&self) -> (&str, &str) {
        CONTENT_NAMES
    }

    fn type_name(&self) -> &str {
        CONTENT_TYPE_NAME
    }

    fn type_mapping(&self) -> &TypeMapping {
        &CONTENT_TYPE_MAPPING
    }
}
