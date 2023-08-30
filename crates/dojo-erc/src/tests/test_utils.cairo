use starknet::ContractAddress;
use starknet::testing::{set_contract_address, set_account_contract_address};

fn impersonate(address: ContractAddress) {
    // world.cairo uses account_contract_address :
    // - in constructor to define world owner
    // - in assert_can_write to check ownership of world & component

    set_account_contract_address(address);
    set_contract_address(address);
}
