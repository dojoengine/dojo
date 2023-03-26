#[system]
mod ERC20_Transfer {
    execute(token_id: felt252, recipient: ContractAddress, value: u256) {
        let caller = get_caller_address();
        assert(!sender.is_zero(), 'ERC20: transfer from 0');
        assert(!recipient.is_zero(), 'ERC20: transfer to 0');

        let ownership_sender_sk: StorageKey = (token_id, (caller, spender)).into();
        let ownership_receiver_sk: StorageKey = (token_id, (caller, spender)).into();
        let ownership = commands::<Ownership>::get(ownership_sk);
        //add storage

        commands::set(ownership_sender_sk, (
            Ownership { balance : ownership.balance - amount}
        ));

        commands::set(ownership_receiver_sk, (
            Ownership { balance : ownership.balance + amount}
        ));

        //this is the event
        Transfer(sender, recipient, amount);
    }
}