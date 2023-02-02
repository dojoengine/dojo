// TODO: I think these should return starknet::ContractAddress but
// I'm not sure how to cast that to a felt.
fn deploy(
    class_hash: felt,
    contract_address_salt: felt,
    constructor_calldata: Array::<felt>,
    deploy_from_zero: bool
) -> felt {
    0x420
}

fn get_contract_address() -> felt {
    0x420
}
