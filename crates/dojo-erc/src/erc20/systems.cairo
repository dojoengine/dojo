#[system]
mod ERC20_Approve {
    use traits::Into;
    use starknet::ContractAddress;
    use dojo_core::storage::key::StorageKey;

    execute(token_id: felt252, spender: ContractAddress, amount: felt252) {
        let caller = starknet::get_caller_address();
        let approval_sk: StorageKey = (token_id, (caller.into(), spender)).into();
        let approval = commands::<Approval>::get(approval_sk);
        commands::set(approval_sk, (
            Approval { amount: amount }
        ))
    }
}

#[system]
mod ERC20_TransferFrom {
    use traits::Into;
    use starknet::ContractAddress;

    use dojo_core::storage::key::StorageKey;

    fn execute(token_address: ContractAddress, spender: ContractAddress, recipient: ContractAddress, amount: felt252) {
        assert(!sender.is_zero(), 'ERC20: transfer from 0');
        assert(!recipient.is_zero(), 'ERC20: transfer to 0');

        let spender_ownership_sk: StorageKey = (token_address, (spender)).into();
        let recipient_ownership_sk: StorageKey = (token_address, (recipient)).into();

        let spen_ownershipder = commands::<Ownership>::get(spender_ownership_sk);
        commands::set(spender_ownership_sk, (
            Ownership { balance : spen_ownershipder.balance - amount}
        ));

        let recipient_ownership = commands::<Ownership>::get(recipient_ownership_sk);
        commands::set(recipient_ownership_sk, (
            Ownership { balance : ownership.balance + amount}
        ));

        //update allowance
        let approval_sk: StorageKey = (token_address, (caller.into(), spender)).into();
        let approval = commands::<Approval>::get(approval_sk);
        
        commands::set(approval_sk, (
            Approval { amount: approval.amount - amount }
        ))
    }
}
