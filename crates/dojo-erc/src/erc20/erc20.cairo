#[starknet::contract]
mod ERC20 {
    use array::ArrayTrait;
    use clone::Clone;
    use integer::BoundedInt;
    use option::OptionTrait;
    use starknet::{ContractAddress, get_caller_address, get_contract_address};
    use traits::{Into, TryInto};
    use zeroable::Zeroable;

    use dojo::world::{IWorldDispatcher, IWorldDispatcherTrait};
    use dojo_erc::erc20::components::{ERC20AllowanceTrait, ERC20BalanceTrait, ERC20SupplyTrait};
    use dojo_erc::erc20::interface::IERC20;
    use dojo_erc::erc20::systems::{
        ERC20Approve, ERC20ApproveParams, ERC20DecreaseAllowance, ERC20DecreaseAllowanceParams,
        ERC20IncreaseAllowance, ERC20IncreaseAllowanceParams, ERC20Mint, ERC20MintParams,
        ERC20TransferFrom, ERC20TransferFromParams
    };
    use dojo_erc::erc_common::utils::{to_calldata, ToCallDataTrait, system_calldata};

    #[storage]
    struct Storage {
        world: IWorldDispatcher,
        name_: felt252,
        symbol_: felt252,
        decimals_: u8,
    }

    #[derive(Clone, Drop, Serde, PartialEq, starknet::Event)]
    struct Transfer {
        from: ContractAddress,
        to: ContractAddress,
        value: u256
    }

    #[derive(Clone, Drop, Serde, PartialEq, starknet::Event)]
    struct Approval {
        owner: ContractAddress,
        spender: ContractAddress,
        value: u256
    }

    #[event]
    #[derive(Drop, PartialEq, starknet::Event)]
    enum Event {
        Transfer: Transfer,
        Approval: Approval
    }

    #[starknet::interface]
    trait IERC20Events<ContractState> {
        fn on_transfer(ref self: ContractState, event: Transfer);
        fn on_approval(ref self: ContractState, event: Approval);
    }

    #[constructor]
    fn constructor(
        ref self: ContractState,
        world: IWorldDispatcher,
        name: felt252,
        symbol: felt252,
        decimals: u8,
        initial_supply: felt252,
        recipient: ContractAddress
    ) {
        self.world.write(world);
        self.name_.write(name);
        self.symbol_.write(symbol);
        self.decimals_.write(decimals);
        if !initial_supply.is_zero() {
            self
                .world
                .read()
                .execute(
                    'ERC20Mint',
                    system_calldata(
                        ERC20MintParams {
                            token: get_contract_address(),
                            recipient,
                            amount: initial_supply.try_into().unwrap()
                        }
                    )
                );
        }
    }


    #[external(v0)]
    impl ERC20 of IERC20<ContractState> {
        fn name(self: @ContractState) -> felt252 {
            self.name_.read()
        }

        fn symbol(self: @ContractState) -> felt252 {
            self.symbol_.read()
        }

        fn decimals(self: @ContractState) -> u8 {
            self.decimals_.read()
        }

        fn total_supply(self: @ContractState) -> u256 {
            ERC20SupplyTrait::total_supply(self.world.read(), get_contract_address()).into()
        }

        fn balance_of(self: @ContractState, account: ContractAddress) -> u256 {
            ERC20BalanceTrait::balance_of(self.world.read(), get_contract_address(), account).into()
        }

        fn allowance(
            self: @ContractState, owner: ContractAddress, spender: ContractAddress
        ) -> u256 {
            ERC20AllowanceTrait::allowance(
                self.world.read(), get_contract_address(), owner, spender
            )
                .into()
        }

        fn approve(ref self: ContractState, spender: ContractAddress, amount: u256) -> bool {
            self
                .world
                .read()
                .execute(
                    'ERC20Approve',
                    system_calldata(
                        ERC20ApproveParams {
                            token: get_contract_address(),
                            caller: get_caller_address(),
                            spender,
                            amount: amount.try_into().unwrap()
                        }
                    )
                );
            true
        }

        fn transfer(ref self: ContractState, recipient: ContractAddress, amount: u256) -> bool {
            self
                .world
                .read()
                .execute(
                    'ERC20TransferFrom',
                    system_calldata(
                        ERC20TransferFromParams {
                            token: get_contract_address(),
                            sender: get_caller_address(),
                            caller: get_caller_address(),
                            recipient,
                            amount: amount.try_into().unwrap()
                        }
                    )
                );
            true
        }

        fn transfer_from(
            ref self: ContractState,
            sender: ContractAddress,
            recipient: ContractAddress,
            amount: u256
        ) -> bool {
            self
                .world
                .read()
                .execute(
                    'ERC20TransferFrom',
                    system_calldata(
                        ERC20TransferFromParams {
                            token: get_contract_address(),
                            sender,
                            caller: get_caller_address(),
                            recipient,
                            amount: amount.try_into().unwrap()
                        }
                    )
                );
            true
        }

        fn increase_allowance(
            ref self: ContractState, spender: ContractAddress, added_value: u256
        ) -> bool {
            self
                .world
                .read()
                .execute(
                    'ERC20IncreaseAllowance',
                    system_calldata(
                        ERC20IncreaseAllowanceParams {
                            token: get_contract_address(),
                            caller: get_caller_address(),
                            spender,
                            added_value: added_value.try_into().unwrap()
                        }
                    )
                );
            true
        }

        fn decrease_allowance(
            ref self: ContractState, spender: ContractAddress, subtracted_value: u256
        ) -> bool {
            self
                .world
                .read()
                .execute(
                    'ERC20DecreaseAllowance',
                    system_calldata(
                        ERC20DecreaseAllowanceParams {
                            token: get_contract_address(),
                            caller: get_caller_address(),
                            spender,
                            subtracted_value: subtracted_value.try_into().unwrap()
                        }
                    )
                );
            true
        }
    }

    #[external(v0)]
    impl ERC20EventEmitter of IERC20Events<ContractState> {
        fn on_transfer(ref self: ContractState, event: Transfer) {
            assert(get_caller_address() == self.world.read().executor(), 'ERC20: not authorized');
            self.emit(event);
        }
        fn on_approval(ref self: ContractState, event: Approval) {
            assert(get_caller_address() == self.world.read().executor(), 'ERC20: not authorized');
            self.emit(event);
        }
    }
}

