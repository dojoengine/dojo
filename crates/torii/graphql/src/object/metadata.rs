use async_graphql::dynamic::Field;

use super::{ObjectTrait, TypeMapping};
use crate::mapping::METADATA_TYPE_MAPPING;
use crate::query::constants::METADATA_TABLE;

pub struct MetadataObject;

impl ObjectTrait for MetadataObject {
    fn name(&self) -> (&str, &str) {
        ("metadata", "metadatas")
    }

    fn type_name(&self) -> &str {
        "Metadata"
    }

    fn type_mapping(&self) -> &TypeMapping {
        &METADATA_TYPE_MAPPING
    }

    fn table_name(&self) -> Option<&str> {
        Some(METADATA_TABLE)
    }

    fn related_fields(&self) -> Option<Vec<Field>> {
        None
    }
}
