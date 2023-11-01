use super::{ObjectTrait, TypeMapping};
use crate::constants::{METADATA_NAMES, METADATA_TABLE, METADATA_TYPE_NAME};
use crate::mapping::METADATA_TYPE_MAPPING;

pub struct MetadataObject;

impl ObjectTrait for MetadataObject {
    fn name(&self) -> (&str, &str) {
        METADATA_NAMES
    }

    fn type_name(&self) -> &str {
        METADATA_TYPE_NAME
    }

    fn type_mapping(&self) -> &TypeMapping {
        &METADATA_TYPE_MAPPING
    }

    fn table_name(&self) -> Option<&str> {
        Some(METADATA_TABLE)
    }
}
