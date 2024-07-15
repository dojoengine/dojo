use super::TypeMapping;
use crate::constants::{SOCIAL_NAMES, SOCIAL_TYPE_NAME};
use crate::mapping::SOCIAL_TYPE_MAPPING;
use crate::object::BasicObject;

#[derive(Debug)]
pub struct SocialObject;

impl BasicObject for SocialObject {
    fn name(&self) -> (&str, &str) {
        SOCIAL_NAMES
    }

    fn type_name(&self) -> &str {
        SOCIAL_TYPE_NAME
    }

    fn type_mapping(&self) -> &TypeMapping {
        &SOCIAL_TYPE_MAPPING
    }
}
