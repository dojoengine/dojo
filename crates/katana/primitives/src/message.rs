use crate::contract::ContractAddress;
use crate::FieldElement;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OrderedL2ToL1Message {
    pub order: u64,
    pub from_address: ContractAddress,
    pub to_address: FieldElement,
    pub payload: Vec<FieldElement>,
}
