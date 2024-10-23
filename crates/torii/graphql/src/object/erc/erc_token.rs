use crate::constants::{
    ERC721_METADATA_NAME, ERC721_METADATA_TYPE_NAME, TOKEN_NAME, TOKEN_TYPE_NAME,
};
use crate::mapping::{ERC721_METADATA_TYPE_MAPPING, TOKEN_TYPE_MAPPING};
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

#[derive(Debug)]
pub struct Erc721MetadataObject;

impl BasicObject for Erc721MetadataObject {
    fn name(&self) -> (&str, &str) {
        ERC721_METADATA_NAME
    }

    fn type_name(&self) -> &str {
        ERC721_METADATA_TYPE_NAME
    }

    fn type_mapping(&self) -> &TypeMapping {
        &ERC721_METADATA_TYPE_MAPPING
    }
}
