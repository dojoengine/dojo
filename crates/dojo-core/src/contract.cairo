/// Call dojo_resource on a contract
///
/// # Arguments
///
/// * `contract_address` - Contract Address of the contract to call dojo_resource on.
fn get_dojo_resource(
    contract_address: starknet::ContractAddress
) -> starknet::SyscallResult<felt252> {
    let dojo_resource = *starknet::call_contract_syscall(
        contract_address, selector!("dojo_resource"), array![].span()
    )?[0];
    Result::Ok(dojo_resource)
}
