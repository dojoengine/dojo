use crate::constants::{ERC721_BALANCE_NAMES, ERC721_BALANCE_TYPE_NAME};
use crate::mapping::ERC721_BALANCE_TYPE_MAPPING;
use crate::object::BasicObject;
use crate::types::TypeMapping;

#[derive(Debug)]
pub struct Erc721BalanceObject;

impl BasicObject for Erc721BalanceObject {
    fn name(&self) -> (&str, &str) {
        ERC721_BALANCE_NAMES
    }

    fn type_name(&self) -> &str {
        ERC721_BALANCE_TYPE_NAME
    }

    fn type_mapping(&self) -> &TypeMapping {
        &ERC721_BALANCE_TYPE_MAPPING
    }
}
