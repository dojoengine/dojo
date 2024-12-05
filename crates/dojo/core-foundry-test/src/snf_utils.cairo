use starknet::{ClassHash, ContractAddress};
use snforge_std::{ContractClassTrait, DeclareResultTrait};
use snforge_std::cheatcodes::contract_class::ContractClass;

pub fn declare(name: ByteArray) -> (ContractClass, ClassHash) {
    let contract = snforge_std::declare(name).unwrap().contract_class();
    (*contract, (*contract.class_hash).into())
}

pub fn deploy(contract: ContractClass, calldata: @Array<felt252>) -> ContractAddress {
    let (address, _) = contract.deploy(calldata).unwrap();
    address
}

pub fn declare_and_deploy(name: ByteArray) -> ContractAddress {
    let contract = snforge_std::declare(name).unwrap().contract_class();
    let (address, _) = contract.deploy(@array![]).unwrap();
    address
}

pub fn declare_contract(name: ByteArray) -> ClassHash {
    let (_, class_hash) = declare(name.clone());
    class_hash
}

pub fn declare_event_contract(name: ByteArray) -> ClassHash {
    declare_contract(format!("e_{name}"))
}

pub fn declare_model_contract(name: ByteArray) -> ClassHash {
    declare_contract(format!("m_{name}"))
}

pub fn set_account_address(account: ContractAddress) {
    snforge_std::start_cheat_account_contract_address_global(account);
}

pub fn set_caller_address(contract: ContractAddress) {
    snforge_std::start_cheat_caller_address_global(contract);
}
