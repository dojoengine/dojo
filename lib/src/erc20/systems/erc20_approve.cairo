use traits::Into;

#[system]
mod ERC20_Approve {
    execute(token_id: felt252, spender: ContractAddress, amount: u256) {
        let caller = get_caller_address();
        let approval_sk: StorageKey = (token_id, (caller.into(), spender)).into();
        let approval = commands::<Approval>::get(approval_sk);
        commands::set(approval_sk, (
            Approval { amount: amount }
        ))
    }
}
