use starknet::ContractAddress;

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

pub fn set_block_number(block_number: u64) {
    snforge_std::start_cheat_block_number_global(block_number);
}
