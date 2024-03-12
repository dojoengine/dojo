
use std::collections::HashSet;

use ethers::types::U256;
use starknet::core::types::{
    ContractStorageDiffItem, DeclaredClassItem, DeployedContractItem, FieldElement, NonceUpdate,
    StateDiff,
};

// 2 ^ 128
const CLASS_INFO_FLAG_TRUE: &str = "0x100000000000000000000000000000000";

