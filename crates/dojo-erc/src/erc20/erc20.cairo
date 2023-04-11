use traits::Into;
use option::OptionTrait;

impl U256TryIntoFelt252 of TryInto::<u256, felt252> {
    fn try_into(self: u256) -> Option<felt252> {
        let low: felt252 = self.low.into();
        let high: felt252 = self.low.into();
        // TODO: bounds checking
        let result : Option<felt252> = Option::Some(low + high);
        match result {
            Option::Some(val) => Option::Some(val),
            Option::None(_) => Option::None(()),
        }
    }
}

#[contract]
mod ERC20 {
    use dojo_core::world;
    use array::ArrayTrait;
    use integer::Felt252IntoU256;
    use traits::Into;
    use dojo_core::storage::query::Query;
    use dojo_core::storage::query::TupleSize2IntoPartitionedQuery;
    use dojo_core::storage::query::TupleSize1IntoPartitionedQuery;
    use dojo_core::interfaces::IWorldDispatcher;
    use dojo_core::interfaces::IWorldDispatcherTrait;
    use dojo_erc::erc20::components::Ownership;
    use zeroable::Zeroable;
    use starknet::get_caller_address;
    use starknet::contract_address_const;
    use starknet::ContractAddress;
    use starknet::ContractAddressZeroable;
    use starknet::contract_address::ContractAddressIntoFelt252;
    use array::SpanTrait;

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
        //balances::read(account)
        //IWorldDispatcher { contract_address: world_address::read() }.entity('Ownership', account.into(),0_u8,0_usize).balance
        let token_id = starknet::get_contract_address();
        let query: Query = (token_id.into(), (account.into(),)).into();
        IWorldDispatcher { contract_address: world_address::read() }.entity('Ownership', query,0_u8,0_usize)
    }

    #[view]
    fn allowance(owner: ContractAddress, spender: ContractAddress) -> u256 {
        let token_id = starknet::get_contract_address();
        let query: Query = (token_id.into(), (owner.into(), spender.into())).into();
        //IWorldDispatcher { contract_address: world_address::read() }.entity('Allowance', query.into(), 0_u8, 0_usize).amount
        IWorldDispatcher { contract_address: world_address::read() }.entity('Allowance', query, 0_u8, 0_usize)
    }

    #[external]
    fn transfer(spender: ContractAddress, recipient: ContractAddress, amount: u256) {
        //ERC20_Transfer.execute(symbol,spender, recipient, amount);

        let mut calldata = ArrayTrait::<felt252>::new();
        calldata.append(starknet::get_contract_address().into());
        calldata.append(spender.into());
        calldata.append(recipient.into());
        calldata.append(amount.into());

        IWorldDispatcher { contract_address: world_address::read() }.execute('ERC20_TransferFrom', calldata.span());
        let token_id = starknet::get_contract_address();

        let approval_sk: Query = (token_id.into(), (recipient.into(), spender.into())).into();
        let approval = commands::<Approval>::entity(approval_sk);

        Transfer(spender, recipient, amount);
        Approval(get_caller_address(),spender,approval.amount);
    }

    #[external]
    fn approve(spender: ContractAddress, amount: u256) {
        let mut calldata = ArrayTrait::<felt252>::new();
        calldata.append(starknet::get_contract_address().into());
        calldata.append(spender.into());
        calldata.append(amount.try_into());

        IWorldDispatcher { contract_address: world_address::read() }.execute('ERC20_Approve', calldata.span());
        Approval(get_caller_address(),spender,amount);
    }
}
