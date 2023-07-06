#[starknet::contract]
mod ERC721 {
    use array::ArrayTrait;
    use option::OptionTrait;
    use starknet::{ContractAddress, get_caller_address, get_contract_address};
    use traits::{Into, TryInto};
    use zeroable::Zeroable;

    use dojo::database::query::{
        Query, LiteralIntoQuery, TupleSize1IntoQuery, TupleSize2IntoQuery, IntoPartitioned,
        IntoPartitionedQuery
    };

    use dojo::world::{IWorldDispatcher, IWorldDispatcherTrait};
    use dojo_erc::erc721::components::{
        Balances, OperatorApprovals, Owners, TokenApprovals, TokenUri
    };
    use dojo_erc::erc721::systems::{
        erc721_approve, erc721_set_approval_for_all, erc721_transfer_from
    };

    #[storage]
    struct Storage {
        world_address: ContractAddress,
        _name: felt252,
        _symbol: felt252,
        _uri: felt252,
    }

    #[event]
    #[derive(Drop, starknet::Event)]
    enum Event {
        Transfer: Transfer,
        Approval: Approval,
        ApprovalForAll: ApprovalForAll
    }

    #[derive(Drop, starknet::Event)]
    struct Transfer {
        from: ContractAddress,
        to: ContractAddress,
        token_id: u256
    }

    #[derive(Drop, starknet::Event)]
    struct Approval {
        owner: ContractAddress,
        to: ContractAddress,
        token_id: u256
    }

    #[derive(Drop, starknet::Event)]
    struct ApprovalForAll {
        owner: ContractAddress,
        operator: ContractAddress,
        approved: bool
    }

    #[constructor]
    fn constructor(
        ref self: ContractState, world: ContractAddress, name: felt252, symbol: felt252
    ) {
        self.world_address.write(world);
        self._name.write(name);
        self._symbol.write(symbol);
    }

    #[external(v0)]
    fn name(self: @ContractState) -> felt252 {
        self._name.read()
    }

    #[external(v0)]
    fn symbol(self: @ContractState) -> felt252 {
        self._symbol.read()
    }

    #[external(v0)]
    fn uri(self: @ContractState) -> felt252 {
        self._uri.read()
    }

    #[external(v0)]
    fn balance_of(self: @ContractState, account: ContractAddress) -> u256 {
        let token = get_contract_address();
        let query: Query = (token, (account, )).into_partitioned();
        let mut balance_raw = world(self).entity('Balances'.into(), query, 0, 0);
        let balance = serde::Serde::<Balances>::deserialize(ref balance_raw).unwrap();
        balance.amount.into()
    }

    #[external(v0)]
    fn owner_of(self: @ContractState, token_id: u256) -> ContractAddress {
        let token = get_contract_address();
        let query: Query = (token, (u256_into_felt252(token_id), )).into_partitioned();
        let mut owner_raw = world(self).entity('Owners'.into(), query, 0, 0);
        let owner = serde::Serde::<Owners>::deserialize(ref owner_raw).unwrap();
        owner.address.try_into().unwrap()
    }

    #[external(v0)]
    fn get_approved(self: @ContractState, token_id: u256) -> ContractAddress {
        let token = get_contract_address();
        let query: Query = (token, (u256_into_felt252(token_id), )).into_partitioned();
        let mut approved_raw = world(self).entity('TokenApprovals'.into(), query, 0, 0);
        let approved = serde::Serde::<TokenApprovals>::deserialize(ref approved_raw).unwrap();
        approved.address.try_into().unwrap()
    }

    #[external(v0)]
    fn is_approved_for_all(
        self: @ContractState, owner: ContractAddress, operator: ContractAddress
    ) -> bool {
        let token = get_contract_address();
        let query: Query = (token, (owner, operator)).into_partitioned();
        let mut result_raw = world(self).entity('OperatorApprovals'.into(), query, 0, 0);
        let result = serde::Serde::<OperatorApprovals>::deserialize(ref result_raw).unwrap();
        felt252_into_bool(result.approved)
    }

    #[external(v0)]
    fn approve(ref self: ContractState, to: ContractAddress, token_id: u256) {
        let mut calldata = ArrayTrait::new();
        let owner = owner_of(@self, token_id);
        calldata.append(u256_into_felt252(token_id));
        calldata.append(to.into());
        world(@self).execute('erc721_approve'.into(), calldata.span());
        let owner = owner_of(@self, token_id);
        self.emit(Approval { owner, to, token_id });
    }

    #[external(v0)]
    fn set_approval_for_all(ref self: ContractState, operator: ContractAddress, approved: bool) {
        let mut calldata = ArrayTrait::new();
        let owner = get_caller_address();
        assert(owner != operator, 'ERC721: approval to owner');
        calldata.append(owner.into());
        calldata.append(operator.into());
        calldata.append(bool_into_felt252(approved));
        world(@self).execute('erc721_set_approval_for_all'.into(), calldata.span());
        self.emit(ApprovalForAll { owner, operator, approved });
    }

    #[external(v0)]
    fn transfer_from(
        ref self: ContractState, from: ContractAddress, to: ContractAddress, token_id: u256
    ) {
        let owner = owner_of(@self, token_id);
        let mut calldata = ArrayTrait::new();
        calldata.append(from.into());
        calldata.append(to.into());
        calldata.append(u256_into_felt252(token_id));
        world(@self).execute('erc721_transfer_from'.into(), calldata.span());
        self.emit(Transfer { from, to, token_id });
    }

    // NOTE: temporary, until we have inline commands outside of systems
    fn world(self: @ContractState) -> IWorldDispatcher {
        IWorldDispatcher { contract_address: self.world_address.read() }
    }

    fn u256_into_felt252(val: u256) -> felt252 {
        // temporary, until TryInto of this is in corelib
        val.low.into() + val.high.into() * 0x100000000000000000000000000000000
    }


    fn bool_into_felt252(_bool: bool) -> felt252 {
        if _bool == true {
            return 1;
        } else {
            return 0;
        }
    }

    fn felt252_into_bool(bool_felt252: felt252) -> bool {
        if bool_felt252 == 1 {
            true
        } else {
            false
        }
    }
}

