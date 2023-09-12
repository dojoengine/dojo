//    * use ufelt when available

#[starknet::contract]
mod ERC20 {
    use array::ArrayTrait;
    use integer::BoundedInt;
    use option::OptionTrait;
    use starknet::{ContractAddress, get_caller_address, get_contract_address};
    use traits::{Into, TryInto};
    use zeroable::Zeroable;

    use dojo::world::{IWorldDispatcher, IWorldDispatcherTrait};
    use dojo_erc::erc20::components::{Allowance, Balance, Supply};
    use dojo_erc::erc20::interface::IERC20;

    const UNLIMITED_ALLOWANCE: felt252 =
        3618502788666131213697322783095070105623107215331596699973092056135872020480;

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
    impl ERC20 of IERC20<ContractState> {
        fn name(self: @ContractState) -> felt252 {
            self.token_name.read()
        }

        fn symbol(self: @ContractState) -> felt252 {
            self.token_symbol.read()
        }

        fn decimals(self: @ContractState) -> u8 {
            self.token_decimals.read()
        }

        fn total_supply(self: @ContractState) -> u256 {
            let contract_address = get_contract_address();
            let supply = get!(self.world.read(), contract_address, Supply);
            supply.amount.into()
        }

        fn balance_of(self: @ContractState, account: ContractAddress) -> u256 {
            let token = get_contract_address();
            let balance = get!(self.world.read(), (token, account), Balance);
            balance.amount.into()
        }

        fn allowance(
            self: @ContractState, owner: ContractAddress, spender: ContractAddress
        ) -> u256 {
            let token = get_contract_address();
            let allowance = get!(self.world.read(), (token, owner, spender), Allowance);

            if (allowance.amount == UNLIMITED_ALLOWANCE) {
                return BoundedInt::max();
            }

            allowance.amount.into()
        }

        fn approve(ref self: ContractState, spender: ContractAddress, amount: u256) -> bool {
            let owner = get_caller_address();
            self._approve(owner, spender, amount);
            true
        }

        fn transfer(ref self: ContractState, recipient: ContractAddress, amount: u256) -> bool {
            let sender = get_caller_address();
            self._transfer(sender, recipient, amount);
            true
        }

        fn transfer_from(
            ref self: ContractState,
            sender: ContractAddress,
            recipient: ContractAddress,
            amount: u256
        ) -> bool {
            let caller = get_caller_address();
            self._spend_allowance(sender, caller, amount);
            self._transfer(sender, recipient, amount);
            true
        }
    }

    //
    // Internal
    //
    #[generate_trait]
    impl InternalImpl of InternalTrait {
        fn _approve(
            ref self: ContractState, owner: ContractAddress, spender: ContractAddress, amount: u256
        ) {
            assert(!owner.is_zero(), 'ERC20: approve from 0');
            assert(!spender.is_zero(), 'ERC20: approve to 0');
            let token = get_contract_address();
            let mut calldata: Array<felt252> = array![
                token.into(), owner.into(), spender.into(), self.u256_as_allowance(amount)
            ];
            self.world.read().execute('erc20_approve', calldata);

            self.emit(Approval { owner, spender, value: amount });
        }

        fn _transfer(
            ref self: ContractState,
            sender: ContractAddress,
            recipient: ContractAddress,
            amount: u256
        ) {
            assert(!sender.is_zero(), 'ERC20: transfer from 0');
            assert(!recipient.is_zero(), 'ERC20: transfer to 0');
            assert(ERC20::balance_of(@self, sender) >= amount, 'ERC20: not enough balance');

            let token = get_contract_address();
            let mut calldata: Array<felt252> = array![
                token.into(), sender.into(), recipient.into(), amount.try_into().unwrap()
            ];
            self.world.read().execute('erc20_transfer_from', calldata);

            self.emit(Transfer { from: Zeroable::zero(), to: recipient, value: amount });
        }

        fn _spend_allowance(
            ref self: ContractState, owner: ContractAddress, spender: ContractAddress, amount: u256
        ) {
            let current_allowance = ERC20::allowance(@self, owner, spender);
            if current_allowance != BoundedInt::max() {
                self._approve(owner, spender, current_allowance - amount);
            }
        }

        fn u256_as_allowance(ref self: ContractState, val: u256) -> felt252 {
            // by convention, max(u256) means unlimited amount,
            // but since we're using felts, use max(felt252) to do the same
            if val == BoundedInt::max() {
                return UNLIMITED_ALLOWANCE;
            }
            val.try_into().unwrap()
        }
    }
}
