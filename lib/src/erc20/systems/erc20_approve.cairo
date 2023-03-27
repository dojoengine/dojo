
#[system]
mod ERC20_Approve {
    use traits::Into;
    use starknet::ContractAddress;

    use dojo::storage::key::StorageKey;

    execute(token_id: felt252, spender: ContractAddress, amount: felt252) {
        let caller = starknet::get_caller_address();
        let approval_sk: StorageKey = (token_id, (caller.into(), spender)).into();
        let approval = commands::<Approval>::get(approval_sk);
        commands::set(approval_sk, (
            Approval { amount: amount }
        ))
    }
}
