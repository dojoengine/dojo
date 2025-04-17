use snforge_std::cheatcodes::contract_class::ContractClass;
use snforge_std::{ContractClassTrait, DeclareResultTrait};
use starknet::{ClassHash, ContractAddress};

/// Declare a contract.
///
/// # Arguments
///   * `name` - the contract name.
///
/// # Returns
///   The declared contract class and classHash.
pub fn declare(name: ByteArray) -> (ContractClass, ClassHash) {
    let contract = snforge_std::declare(name).unwrap().contract_class();
    (*contract, (*contract.class_hash).into())
}

/// Deploy a contract.
///
/// # Arguments
///   * `contract_class` - the contract class.
///   * `calldata` - serialized calldata to pass to constructor.
///
/// # Returns
///   The deployed contract address.
pub fn deploy(contract_class: ContractClass, calldata: @Array<felt252>) -> ContractAddress {
    let (address, _) = contract_class.deploy(calldata).unwrap();
    address
}

/// Declare and deploy and contract.
///
/// # Arguments
///   * `name` - the contract name.
///
/// # Returns
///   The deployed contract address.
pub fn declare_and_deploy(name: ByteArray) -> ContractAddress {
    let contract = snforge_std::declare(name).unwrap().contract_class();
    let (address, _) = contract.deploy(@array![]).unwrap();
    address
}

/// Declare a Dojo contract.
///
/// # Arguments
///   * `name` - the contract name.
///
/// # Returns
///   The declared contract classhash.
pub fn declare_contract(name: ByteArray) -> ClassHash {
    let (_, class_hash) = declare(name.clone());
    class_hash
}

/// Declare a Dojo library.
///
/// # Arguments
///   * `name` - the library contract name.
///
/// # Returns
///   The declared library classhash.
pub fn declare_library(name: ByteArray) -> ClassHash {
    declare_contract(name)
}

/// Declare a Dojo Event contract.
///
/// # Arguments
///   * `name` - the contract name.
///
/// # Returns
///   The declared contract classhash.
pub fn declare_event_contract(name: ByteArray) -> ClassHash {
    declare_contract(format!("e_{name}"))
}

/// Declare a Dojo Model contract.
///
/// # Arguments
///   * `name` - the contract name.
///
/// # Returns
///   The declared contract classhash.
pub fn declare_model_contract(name: ByteArray) -> ClassHash {
    declare_contract(format!("m_{name}"))
}

/// Set the global account address used for tests.
///
/// # Arguments
///   * `account` - the account address.
pub fn set_account_address(account: ContractAddress) {
    snforge_std::start_cheat_account_contract_address_global(account);
}

/// Set the global caller address used for tests.
///
/// # Arguments
///   * `account` - the caller address.
pub fn set_caller_address(contract: ContractAddress) {
    snforge_std::start_cheat_caller_address_global(contract);
}

/// Get the default caller address used for tests.
pub fn get_default_caller_address() -> ContractAddress {
    snforge_std::test_address()
}
