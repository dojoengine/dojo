#[system]
mod ERC20_Transfer {
    execute(token_id: felt252,spender:ContractAddress, recipient: ContractAddress, value: u256) {
        assert(!sender.is_zero(), 'ERC20: transfer from 0');
        assert(!recipient.is_zero(), 'ERC20: transfer to 0');

        let ownership_spender_sk: StorageKey = (token_id, ( spender)).into();
        let ownership_recipient_sk: StorageKey = (token_id, (recipient)).into();

        let ownership_spender = commands::<Ownership>::get(ownership_spender_sk);
        commands::set(ownership_spender_sk, (
            Ownership { balance : ownership_spender.balance - amount}
        ));

        let recipient_ownership = commands::<Ownership>::get(ownership_recipient_sk);
        commands::set(ownership_recipient_sk, (
            Ownership { balance : ownership.balance + amount}
        ));
    }
}
