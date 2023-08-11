// TODO: future improvements when Cairo catches up
//    * use BoundedInt in allowance calc
//    * use inline commands (currently available only in systems)
//    * use ufelt when available

#[starknet::contract]
mod ERC20 {
    // max(felt252)
    const UNLIMITED_ALLOWANCE: felt252 =
        3618502788666131213697322783095070105623107215331596699973092056135872020480;

    use array::ArrayTrait;
    use option::OptionTrait;
    use starknet::{ContractAddress, get_caller_address, get_contract_address};
    use traits::Into;
    use zeroable::Zeroable;

    use dojo::world::{IWorldDispatcher, IWorldDispatcherTrait};
    use dojo_erc::erc20::components::{Allowance, Balance, Supply};

    #[storage]
    struct Storage {
        world_address: ContractAddress,
        token_name: felt252,
        token_symbol: felt252,
        token_decimals: u8,
    }

    #[event]
    #[derive(Drop, starknet::Event)]
    enum Event {
        Transfer: Transfer,
        Approval: Approval
    }


    #[derive(Drop, starknet::Event)]
    struct Transfer {
        from: ContractAddress,
        to: ContractAddress,
        value: u256
    }

    #[derive(Drop, starknet::Event)]
    struct Approval {
        owner: ContractAddress,
        spender: ContractAddress,
        value: u256
    }

    #[constructor]
    fn constructor(
        ref self: ContractState,
        world: ContractAddress,
        name: felt252,
        symbol: felt252,
        decimals: u8,
        initial_supply: felt252,
        recipient: ContractAddress
    ) {
        self.world_address.write(world);
        self.token_name.write(name);
        self.token_symbol.write(symbol);
        self.token_decimals.write(decimals);

        if initial_supply != 0 {
            assert(recipient.is_non_zero(), 'ERC20: mint to 0');
            let token = get_contract_address();
            let mut calldata = ArrayTrait::new();
            calldata.append(token.into());
            calldata.append(recipient.into());
            calldata.append(initial_supply);
            world(@self).execute('ERC20Mint', calldata.span());

            self
                .emit(
                    Transfer { from: Zeroable::zero(), to: recipient, value: initial_supply.into() }
                );
        }
    }

    #[external(v0)]
    fn name(self: @ContractState) -> felt252 {
        self.token_name.read()
    }

    #[external(v0)]
    fn symbol(self: @ContractState) -> felt252 {
        self.token_symbol.read()
    }

    #[external(v0)]
    fn decimals(self: @ContractState) -> u8 {
        self.token_decimals.read()
    }

    #[external(v0)]
    fn total_supply(self: @ContractState) -> u256 {
        let contract_address = get_contract_address();
        let supply = get !(world(self), contract_address, Supply);
        supply.amount.into()
    }

    #[external(v0)]
    fn balance_of(self: @ContractState, account: ContractAddress) -> u256 {
        let token = get_contract_address();
        let mut keys = ArrayTrait::new();
        keys.append(token.into());
        keys.append(account.into());
        let mut balance_raw = world(self).entity('Balance', keys.span(), 0, 0);
        let balance = serde::Serde::<Balance>::deserialize(ref balance_raw).unwrap();
        balance.amount.into()
    }

    #[external(v0)]
    fn allowance(self: @ContractState, owner: ContractAddress, spender: ContractAddress) -> u256 {
        let token = get_contract_address();
        let mut keys = ArrayTrait::new();
        keys.append(token.into());
        keys.append(owner.into());
        keys.append(spender.into());
        let mut allowance_raw = world(self).entity('Allowance', keys.span(), 0, 0);
        let allowance = serde::Serde::<Allowance>::deserialize(ref allowance_raw).unwrap();
        allowance.amount.into()
    }

    #[external(v0)]
    fn approve(ref self: ContractState, spender: ContractAddress, amount: u256) -> bool {
        assert(spender.is_non_zero(), 'ERC20: approve to 0');

        let token = get_contract_address();
        let owner = get_caller_address();
        let mut calldata = ArrayTrait::new();
        calldata.append(token.into());
        calldata.append(owner.into());
        calldata.append(spender.into());
        calldata.append(u256_as_allowance(amount));
        world(@self).execute('ERC20Approve', calldata.span());

        self.emit(Approval { owner, spender, value: amount });

        true
    }

    #[external(v0)]
    fn transfer(ref self: ContractState, recipient: ContractAddress, amount: u256) -> bool {
        transfer_internal(ref self, get_caller_address(), recipient, amount);
        true
    }

    #[external(v0)]
    fn transfer_from(
        ref self: ContractState, spender: ContractAddress, recipient: ContractAddress, amount: u256
    ) -> bool {
        transfer_internal(ref self, spender, recipient, amount);
        true
    }

    //
    // Internal
    //

    // NOTE: temporary, until we have inline commands outside of systems
    fn world(self: @ContractState) -> IWorldDispatcher {
        IWorldDispatcher { contract_address: self.world_address.read() }
    }

    fn transfer_internal(
        ref self: ContractState, spender: ContractAddress, recipient: ContractAddress, amount: u256
    ) {
        assert(recipient.is_non_zero(), 'ERC20: transfer to 0');

        let token = get_contract_address();
        let mut calldata = ArrayTrait::new();
        calldata.append(token.into());
        calldata.append(spender.into());
        calldata.append(recipient.into());
        calldata.append(u256_into_felt252(amount));

        world(@self).execute('ERC20TransferFrom', calldata.span());

        self.emit(Transfer { from: Zeroable::zero(), to: recipient, value: amount });
    }

    fn u256_as_allowance(val: u256) -> felt252 {
        // by convention, max(u256) means unlimited amount,
        // but since we're using felts, use max(felt252) to do the same
        // TODO: use BoundedInt when available
        let max_u128 = 0xffffffffffffffffffffffffffffffff;
        let max_u256 = u256 { low: max_u128, high: max_u128 };
        if val == max_u256 {
            return UNLIMITED_ALLOWANCE;
        }
        u256_into_felt252(val)
    }

    fn u256_into_felt252(val: u256) -> felt252 {
        // temporary, until TryInto of this is in corelib
        val.low.into() + val.high.into() * 0x100000000000000000000000000000000
    }
}