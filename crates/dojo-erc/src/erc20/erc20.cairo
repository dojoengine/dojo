use core::traits::TryInto;
// TODO: future improvements when Cairo catches up
//    * use BoundedInt in allowance calc
//    * use inline commands (currently available only in systems)
//    * use ufelt when available

use starknet::ContractAddress;

#[starknet::interface]
trait IERC20<TState> {
    fn name(self: @TState) -> felt252;
    fn symbol(self: @TState) -> felt252;
    fn decimals(self: @TState) -> u8;
    fn total_supply(self: @TState) -> u256;
    fn balance_of(self: @TState, account: ContractAddress) -> u256;
    fn allowance(self: @TState, owner: ContractAddress, spender: ContractAddress) -> u256;
    fn transfer(ref self: TState, recipient: ContractAddress, amount: u256) -> bool;
    fn transfer_from(
        ref self: TState, sender: ContractAddress, recipient: ContractAddress, amount: u256
    ) -> bool;
    fn approve(ref self: TState, spender: ContractAddress, amount: u256) -> bool;
}

#[starknet::contract]
mod ERC20 {
    // max(felt252)
    const UNLIMITED_ALLOWANCE: felt252 =
        3618502788666131213697322783095070105623107215331596699973092056135872020480;
    use debug::PrintTrait;
    use array::ArrayTrait;
    use box::BoxTrait;
    use integer::BoundedInt;
    use option::OptionTrait;
    use starknet::{
        ContractAddress, ContractAddressIntoFelt252, get_caller_address, get_contract_address,
        get_execution_info
    };
    use traits::{Into, TryInto};
    use zeroable::Zeroable;

    use dojo::world::{IWorldDispatcher, IWorldDispatcherTrait};
    use dojo_erc::erc20::components::{Allowance, Balance, Supply};

    #[storage]
    struct Storage {
        world: IWorldDispatcher,
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
        self.world.write(IWorldDispatcher { contract_address: world });
        self.token_name.write(name);
        self.token_symbol.write(symbol);
        self.token_decimals.write(decimals);
        let mut calldata: Array<felt252> = array![];
        if initial_supply != 0 {
            assert(!recipient.is_zero(), 'ERC20: mint to 0');
            let mut calldata: Array<felt252> = array![];
            let token = get_contract_address();
            calldata.append(token.into());
            calldata.append(recipient.into());
            calldata.append(initial_supply);
            self.world.read().execute('erc20_mint', calldata);

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
        // let supply = get!(self.world.read(), contract_address, Supply);
        let mut keys: Array<felt252> = array![];
        keys.append(contract_address.into());
        // TODO: to be change when global macros are available
        let span = self
            .world
            .read()
            .entity('Supply', keys.span(), 0, dojo::SerdeLen::<Supply>::len());
        let supply = *span[0];
        supply.into()
    }

    #[external(v0)]
    fn balance_of(self: @ContractState, account: ContractAddress) -> u256 {
        let token = get_contract_address();
        // let balance: Balance = get!(self.world.read(), (token, account), Balance);
        let mut keys: Array<felt252> = array![];
        keys.append(token.into());
        keys.append(account.into());
        // TODO: to be change when global macros are available
        let span = self
            .world
            .read()
            .entity('Balance', keys.span(), 0, dojo::SerdeLen::<Balance>::len());
        let balance = *span[0];
        balance.into()
    }
    #[external(v0)]
    fn allowance(self: @ContractState, owner: ContractAddress, spender: ContractAddress) -> u256 {
        let token = get_contract_address();
        // let allowance = get!(self.world.read(), (token, owner, spender), Allowance);
        // allowance.amount.into()
        let mut keys: Array<felt252> = array![];
        keys.append(token.into());
        keys.append(owner.into());
        keys.append(spender.into());
        // TODO: to be change when global macros are available
        let span = self
            .world
            .read()
            .entity('Allowance', keys.span(), 0, dojo::SerdeLen::<Allowance>::len());
        let allowance = *span[0];
        allowance.into()
    }

    #[external(v0)]
    fn approve(ref self: ContractState, spender: ContractAddress, amount: u256) -> bool {
        let owner = get_caller_address();
        _approve(ref self, owner, spender, amount);
        true
    }

    #[external(v0)]
    fn transfer(ref self: ContractState, recipient: ContractAddress, amount: u256) -> bool {
        let sender = get_caller_address();
        _transfer(ref self, sender, recipient, amount);
        true
    }

    #[external(v0)]
    fn transfer_from(
        ref self: ContractState, sender: ContractAddress, recipient: ContractAddress, amount: u256
    ) -> bool {
        let caller = get_caller_address();
        _spend_allowance(ref self, sender, caller, amount);
        _transfer(ref self, sender, recipient, amount);
        true
    }

    //
    // Internal
    //

    fn _approve(
        ref self: ContractState, owner: ContractAddress, spender: ContractAddress, amount: u256
    ) {
        assert(!owner.is_zero(), 'ERC20: approve from 0');
        assert(!spender.is_zero(), 'ERC20: approve to 0');
        let token = get_contract_address();
        let mut calldata: Array<felt252> = array![];
        calldata.append(token.into());
        calldata.append(owner.into());
        calldata.append(spender.into());
        calldata.append(u256_as_allowance(amount));
        self.world.read().execute('erc20_approve', calldata);

        self.emit(Approval { owner, spender, value: amount });
    }

    fn _transfer(
        ref self: ContractState, sender: ContractAddress, recipient: ContractAddress, amount: u256
    ) {
        assert(!sender.is_zero(), 'ERC20: transfer from 0');
        assert(!recipient.is_zero(), 'ERC20: transfer to 0');
        assert(balance_of(@self, sender) >= amount, 'ERC20: not enough balance');

        let token = get_contract_address();
        let mut calldata: Array<felt252> = array![];
        calldata.append(token.into());
        calldata.append(sender.into());
        calldata.append(recipient.into());
        calldata.append(amount.try_into().unwrap());
        self.world.read().execute('erc20_transfer_from', calldata);

        self.emit(Transfer { from: Zeroable::zero(), to: recipient, value: amount });
    }

    fn _spend_allowance(
        ref self: ContractState, owner: ContractAddress, spender: ContractAddress, amount: u256
    ) {
        let current_allowance = allowance(@self, owner, spender);

        if current_allowance != UNLIMITED_ALLOWANCE.into() {
            _approve(ref self, owner, spender, current_allowance - amount);
        }
    }

    fn u256_as_allowance(val: u256) -> felt252 {
        // by convention, max(u256) means unlimited amount,
        // but since we're using felts, use max(felt252) to do the same
        if val == BoundedInt::max() {
            return UNLIMITED_ALLOWANCE;
        }
        val.try_into().unwrap()
    }
}

