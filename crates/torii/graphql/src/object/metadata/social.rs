use async_graphql::dynamic::Field;

use super::{ObjectTrait, TypeMapping};
use crate::constants::{SOCIAL_NAMES, SOCIAL_TYPE_NAME};
use crate::mapping::SOCIAL_TYPE_MAPPING;

pub struct SocialObject;

impl ObjectTrait for SocialObject {
    fn name(&self) -> (&str, &str) {
        SOCIAL_NAMES
    }

    fn type_name(&self) -> &str {
        SOCIAL_TYPE_NAME
    }

    fn type_mapping(&self) -> &TypeMapping {
        &SOCIAL_TYPE_MAPPING
    }

    fn resolve_one(&self) -> Option<Field> {
        None
    }

    fn resolve_many(&self) -> Option<Field> {
        None
    }
}
