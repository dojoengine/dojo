use async_graphql::dynamic::Field;

use super::{ObjectTrait, TypeMapping};
use crate::constants::{CONTENT_NAMES, CONTENT_TYPE_NAME};
use crate::mapping::CONTENT_TYPE_MAPPING;

pub struct ContentObject;

impl ObjectTrait for ContentObject {
    fn name(&self) -> (&str, &str) {
        CONTENT_NAMES
    }

    fn type_name(&self) -> &str {
        CONTENT_TYPE_NAME
    }

    fn type_mapping(&self) -> &TypeMapping {
        &CONTENT_TYPE_MAPPING
    }

    fn resolve_one(&self) -> Option<Field> {
        None
    }

    fn resolve_many(&self) -> Option<Field> {
        None
    }
}
