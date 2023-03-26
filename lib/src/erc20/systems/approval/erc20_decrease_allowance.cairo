#[system]
mod ERC20_Decrease_Allowance {
    execute(token_id: felt252, spender: ContractAddress, substracted_value: u256) {
        let caller = get_caller_address();
        let approval_sk: StorageKey = (token_id, (caller, spender)).into();
        let approval = commands::<Approval>::get(approval_sk);
        commands::set(approval_sk, (
            Approval { amount: approval.amount - substracted_value }
        ))
    }
}