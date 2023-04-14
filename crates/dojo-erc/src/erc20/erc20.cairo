// use traits::Into;
// use option::OptionTrait;

// impl U256TryIntoFelt252 of TryInto::<u256, felt252> {
//     fn try_into(self: u256) -> Option<felt252> {
//         let low: felt252 = self.low.into();
//         let high: felt252 = self.low.into();
//         // TODO: bounds checking
//         let result : Option<felt252> = Option::Some(low + high);
//         match result {
//             Option::Some(val) => Option::Some(val),
//             Option::None(_) => Option::None(()),
//         }
//     }
// }

#[contract]
mod ERC20 {
    use array::ArrayTrait;
    use traits::Into;
    use option::OptionTrait;

    use dojo_core::storage::query::Query;
    use dojo_core::storage::query::ContractAddressIntoQuery;
    use dojo_core::storage::query::TupleSize2IntoPartitionedQuery;
    use dojo_core::storage::query::TupleSize1IntoPartitionedQuery;
    use dojo_core::interfaces::IWorldDispatcher;
    use dojo_core::interfaces::IWorldDispatcherTrait;
    use zeroable::Zeroable;
    use starknet::get_caller_address;
    use starknet::get_contract_address;
    use starknet::ContractAddress;
    use starknet::ContractAddressZeroable;

    use dojo_erc::erc20::components::Allowance;
    use dojo_erc::erc20::components::Balance;
    use dojo_erc::erc20::components::Supply;

    struct Storage {
        world_address: ContractAddress,
        token_name: felt252,
        token_symbol: felt252,
        token_decimals: u8,
    }

    #[event]
    fn Transfer(from: ContractAddress, to: ContractAddress, value: u256) {}

    #[event]
    fn Approval(from: ContractAddress, to: ContractAddress, value: u256) {}

    #[constructor]
    fn constructor(
        world: ContractAddress,
        name: felt252,
        symbol: felt252,
        decimals: u8,
        initial_supply: felt252,
        recipient: ContractAddress
    ) {
        world_address::write(world);
        token_name::write(name);
        token_symbol::write(symbol);
        token_decimals::write(decimals);

        if initial_supply != 0 {
            assert(recipient.is_non_zero(), 'ERC20: mint to the 0 address');
            let token = get_contract_address();
            let mut calldata = ArrayTrait::<felt252>::new();
            calldata.append(token.into());
            calldata.append(recipient.into());
            calldata.append(initial_supply);
            IWorldDispatcher { contract_address: world }.execute(
                'ERC20Mint', calldata.span()
            );
            Transfer(Zeroable::zero(), recipient, initial_supply.into());
        }
    }

    #[view]
    fn name() -> felt252 {
        token_name::read()
    }

    #[view]
    fn symbol() -> felt252 {
        token_symbol::read()
    }

    #[view]
    fn decimals() -> u8 {
        token_decimals::read()
    }

    #[view]
    fn total_supply() -> u256 {
        let query: Query = get_contract_address().into();
        let mut supply_raw = IWorldDispatcher { contract_address: world_address::read() }.entity(
            'Supply', query, 0_u8, 0_usize
        );
        let supply = serde::Serde::<Supply>::deserialize(ref supply_raw).unwrap();
        supply.amount.into()
    }

    #[view]
    fn balance_of(account: ContractAddress) -> u256 {
        let token = get_contract_address();
        let query: Query = (token.into(), (account.into(),)).into();
        
        let mut balance_raw = IWorldDispatcher { contract_address: world_address::read() }.entity(
            'Balance', query, 0_u8, 0_usize
        );
        let balance = serde::Serde::<Balance>::deserialize(ref balance_raw).unwrap();
        balance.amount.into()
    }

    // #[view]
    // fn allowance(owner: ContractAddress, spender: ContractAddress) -> u256 {
    //     let token_id = starknet::get_contract_address();
    //     let query: Query = (token_id.into(), (owner.into(), spender.into())).into();
    //     let approval = IWorldDispatcher { contract_address: world_address::read() }.entity('Approval', query, 0_u8, 0_usize);
    //     if approval.is_empty() {
    //         return 0.into();
    //     }

    //     (*approval.at(0_usize)).into()
    // }

    // #[external]
    // fn transfer(spender: ContractAddress, recipient: ContractAddress, amount: u256) {
    //     let token_id = starknet::get_contract_address();

    //     let mut calldata = ArrayTrait::<felt252>::new();
    //     calldata.append(token_id.into());
    //     calldata.append(spender.into());
    //     calldata.append(recipient.into());
    //     calldata.append(amount.try_into());

    //     IWorldDispatcher { contract_address: world_address::read() }.execute('ERC20_TransferFrom', calldata.span());

    //     // let approval_sk: Query = (token_id.into(), (recipient.into(), spender.into())).into();
    //     // let approval = commands::<Approval>::entity(approval_sk);

    //     // Transfer(spender, recipient, amount);
    //     // Approval(get_caller_address(), spender, approval.amount);
    // }

    // #[external]
    // fn approve(spender: ContractAddress, amount: u256) {
    //     let mut calldata = ArrayTrait::<felt252>::new();
    //     calldata.append(starknet::get_contract_address().into());
    //     calldata.append(spender.into());
    //     calldata.append(amount.try_into());

    //     IWorldDispatcher { contract_address: world_address::read() }.execute('ERC20_Approve', calldata.span());
    //     Approval(get_caller_address(), spender, amount);
    // }
}
