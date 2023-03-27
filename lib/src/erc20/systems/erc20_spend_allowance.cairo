#[system]
mod ERC20_Spend_Allowance {
    use traits::Into;
    use starknet::ContractAddress;
    use dojo::storage::key::StorageKey;

    execute(token_id: felt252,caller: ContractAddress, spender: ContractAddress, amount: felt252) {
    let current_allowance = allowances::read((caller, spender));
            let ONES_MASK = 0xffffffffffffffffffffffffffffffff_u128;
            let is_unlimited_allowance =
                current_allowance.low == ONES_MASK & current_allowance.high == ONES_MASK;
            if !is_unlimited_allowance {
                assert(!spender.is_zero(), 'ERC20: approve from 0');
                let approval_sk: StorageKey = (token_id, (caller.into(), spender)).into();
                let approval = commands::<Approval>::get(approval_sk);
                commands::set(approval_sk, (
                    Approval { amount: current_allowance - amount }
                ))
            }
    }
}