fn deploy(
    class_hash: felt,
    contract_address_salt: felt,
    constructor_calldata: Array::<felt>,
    deploy_from_zero: bool
) -> starknet::ContractAddress {
    starknet::contract_address_const::<0x420>()
}

// NOTE: Not available yet: https://docs.starknet.io/documentation/starknet_versions/upcoming_versions/#replace_class_syscall
fn replace_class(class_hash: felt) {}
