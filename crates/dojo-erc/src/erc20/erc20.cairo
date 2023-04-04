use traits::TryInto;

impl U256TryIntoFelt252 of TryInto::<u256, felt252> {
    fn try_into(self: u256) -> Option<felt252> {
        let low: felt252 = self.low.try_into();
        let high: felt252 = self.low.try_into();
        // TODO: bounds checking
        low + high
    }
}

#[contract]
mod ERC20 {
    use dojo_core::world;
    use dojo_core::storage::key::StorageKey;
    use zeroable::Zeroable;
    use starknet::get_caller_address;
    use starknet::contract_address_const;
    use starknet::ContractAddress;
    use starknet::ContractAddressZeroable;

    struct Storage {
        world_address: ContractAddress,
        name: felt252,
        symbol: felt252,
        decimals: u8,
        total_supply: u256,
    }

    #[event]
    fn Transfer(from: ContractAddress, to: ContractAddress, value: u256) {}

    #[event]
    fn Approval(from: ContractAddress, to: ContractAddress, value: u256) {}

    #[constructor]
    fn constructor(
        world_address_: ContractAddress,
        name_: felt252,
        symbol_: felt252,
        decimals_: u8,
        initial_supply: u256,
        recipient: ContractAddress
    ) {
        world_address::write(world_address_);
        name::write(name_);
        symbol::write(symbol_);
        decimals::write(decimals_);
        assert(!recipient.is_zero(), 'ERC20: mint to the 0 address');
        total_supply::write(initial_supply);

        Transfer(contract_address_const::<0>(), recipient, initial_supply);
    }

    #[view]
    fn get_name() -> felt252 {
        name::read()
    }

    #[view]
    fn get_symbol() -> felt252 {
        symbol::read()
    }

    #[view]
    fn get_decimals() -> u8 {
        decimals::read()
    }

    #[view]
    fn get_total_supply() -> u256 {
        total_supply::read()
    }

    #[view]
    fn balance_of(account: ContractAddress) -> u256 {
        balances::read(account)
    }

    #[view]
    fn allowance(owner: ContractAddress, spender: ContractAddress) -> u256 {
        let token_id = starknet::get_contract_address();
        let key: StorageKey : (token_id.into(), (owner.into(), spender.into())).into();
        IWorldDispatcher { contract_address: world_address::read() }.get('Allowance', key.into(), 0_u8, 0_usize);
    }

    #[external]
    fn transfer(spender: ContractAddress, recipient: ContractAddress, amount: u256) {
        ERC20_Transfer.execute(symbol,spender, recipient, amount);

        let calldata = ArrayTrait::<felt252>::new();
        calldata.append(starknet::get_contract_address().into());
        calldata.append(spender.into());
        calldata.append(recipient.into());
        calldata.append(amount.try_into());

        IWorldDispatcher { contract_address: world_address::read() }.execute('ERC20_TransferFrom', calldata.span());

        let approval_sk: StorageKey = (token_id, (caller.into(), spender)).into();
        let approval = commands::<Approval>::get(approval_sk);

        Transfer(spender, recipient, amount);
        Approval(get_caller_address(),spender,approval.amount);
    }

    #[external]
    fn approve(spender: ContractAddress, amount: u256) {
        let calldata = ArrayTrait::<felt252>::new();
        calldata.append(starknet::get_contract_address().into());
        calldata.append(spender.into());
        calldata.append(amount.try_into());

        IWorldDispatcher { contract_address: world_address::read() }.execute('ERC20_Approve', calldata.span());
        Approval(get_caller_address(),spender,amount);
    }
}
