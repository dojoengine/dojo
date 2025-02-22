use starknet::core::types::contract::SierraClass;
use starknet::core::types::Felt;

#[derive(Debug, Clone)]
pub struct ExternalContractLocal {
    pub contract_name: String,
    pub class_hash: Felt,
    pub instance_name: String,
    pub salt: Felt,
    pub constructor_data: Vec<String>,
    pub raw_constructor_data: Vec<Felt>,
    pub address: Felt,
}

#[derive(Debug, Clone)]
pub struct ExternalContractClassLocal {
    pub contract_name: String,
    pub casm_class_hash: Felt,
    pub class: SierraClass,
}
