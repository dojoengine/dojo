use crate::constants::{ERC20_BALANCE_NAMES, ERC20_BALANCE_TYPE_NAME};
use crate::mapping::ERC20_BALANCE_TYPE_MAPPING;
use crate::object::BasicObject;
use crate::types::TypeMapping;

#[derive(Debug)]
pub struct Erc20BalanceObject;

impl BasicObject for Erc20BalanceObject {
    fn name(&self) -> (&str, &str) {
        ERC20_BALANCE_NAMES
    }

    fn type_name(&self) -> &str {
        ERC20_BALANCE_TYPE_NAME
    }

    fn type_mapping(&self) -> &TypeMapping {
        &ERC20_BALANCE_TYPE_MAPPING
    }
}
