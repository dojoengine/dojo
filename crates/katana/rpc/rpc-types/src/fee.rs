use katana_primitives::ContractAddress;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct FeeToken {
    pub name: String,
    pub address: ContractAddress,
}
