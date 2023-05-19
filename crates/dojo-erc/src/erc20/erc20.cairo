// TODO: future improvements when Cairo catches up
//    * use BoundedInt in allowance calc
//    * use inline commands (currently available only in systems)
//    * use ufelt when available

#[contract]
mod ERC20 {
    // max(felt252)
    const UNLIMITED_ALLOWANCE: felt252 = 3618502788666131213697322783095070105623107215331596699973092056135872020480;

    use array::ArrayTrait;
    use option::OptionTrait;
    use starknet::{ContractAddress, get_caller_address, get_contract_address};
    use traits::Into;
    use zeroable::Zeroable;

    use dojo_core::storage::query::{
        Query,
        LiteralIntoQuery,
        TupleSize1IntoQuery,
        TupleSize2IntoQuery,
        IntoPartitioned,
        IntoPartitionedQuery
    };
    
    use dojo_core::{integer::u250, string::ShortString, interfaces::{IWorldDispatcher, IWorldDispatcherTrait}};
    use dojo_erc::erc20::components::{Allowance, Balance, Supply};

    struct Storage {
        world_address: ContractAddress,
        token_name: felt252,
        token_symbol: felt252,
        token_decimals: u8,
    }

    #[event]
    fn Transfer(from: ContractAddress, to: ContractAddress, value: u256) {}

    #[event]
    fn Approval(owner: ContractAddress, spender: ContractAddress, value: u256) {}

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
            assert(recipient.is_non_zero(), 'ERC20: mint to 0');
            let token = get_contract_address();
            let mut calldata = ArrayTrait::new();
            calldata.append(token.into());
            calldata.append(recipient.into());
            calldata.append(initial_supply);
            world().execute('ERC20Mint'.into(), calldata.span());
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
        let mut supply_raw = world().entity('Supply'.into(), query, 0, 0);
        let supply = serde::Serde::<Supply>::deserialize(ref supply_raw).unwrap();
        supply.amount.into()
    }

    #[view]
    fn balance_of(account: ContractAddress) -> u256 {
        let token = get_contract_address();
        let query: Query = (token, (account,)).into_partitioned();        
        let mut balance_raw = world().entity('Balance'.into(), query, 0, 0);
        let balance = serde::Serde::<Balance>::deserialize(ref balance_raw).unwrap();
        balance.amount.into()
    }

    #[view]
    fn allowance(owner: ContractAddress, spender: ContractAddress) -> u256 {
        let token = get_contract_address();
        let query: Query = (token, (owner, spender)).into_partitioned();
        let mut allowance_raw = world().entity('Allowance'.into(), query, 0, 0);
        let allowance = serde::Serde::<Allowance>::deserialize(ref allowance_raw).unwrap();
        allowance.amount.into()
    }

    #[external]
    fn approve(spender: ContractAddress, amount: u256) -> bool {
        assert(spender.is_non_zero(), 'ERC20: approve to 0');

        let token = get_contract_address();
        let owner = get_caller_address();
        let mut calldata = ArrayTrait::new();
        calldata.append(token.into());
        calldata.append(owner.into());
        calldata.append(spender.into());
        calldata.append(u256_as_allowance(amount));
        world().execute('ERC20Approve'.into(), calldata.span());

        Approval(owner, spender, amount);

        true
    }

    #[external]
    fn transfer(recipient: ContractAddress, amount: u256) -> bool {
        transfer_internal(get_caller_address(), recipient, amount);
        true
    }

    #[external]
    fn transfer_from(spender: ContractAddress, recipient: ContractAddress, amount: u256) -> bool {
        transfer_internal(spender, recipient, amount);
        true
    }

    //
    // Internal
    //

    // NOTE: temporary, until we have inline commands outside of systems
    fn world() -> IWorldDispatcher {
        IWorldDispatcher { contract_address: world_address::read() }
    }

    fn transfer_internal(spender: ContractAddress, recipient: ContractAddress, amount: u256) {
        assert(recipient.is_non_zero(), 'ERC20: transfer to 0');

        let token = get_contract_address();
        let mut calldata = ArrayTrait::new();
        calldata.append(token.into());
        calldata.append(spender.into());
        calldata.append(recipient.into());
        calldata.append(u256_into_felt252(amount));

        world().execute('ERC20TransferFrom'.into(), calldata.span());

        Transfer(spender, recipient, amount);
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
