use crate::contract::ContractAddress;
use crate::Felt;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "arbitrary", derive(::arbitrary::Arbitrary))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OrderedL2ToL1Message {
    pub order: u64,
    pub from_address: ContractAddress,
    pub to_address: Felt,
    pub payload: Vec<Felt>,
}
