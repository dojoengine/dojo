#[starknet::contract]
mod ERC721 {
    use array::ArrayTrait;
    use option::OptionTrait;
    use starknet::{ContractAddress, get_caller_address, get_contract_address};
    use traits::{Into, TryInto};
    use zeroable::Zeroable;

    use dojo::world::{IWorldDispatcher, IWorldDispatcherTrait};
    use dojo_erc::erc721::components::{Balance, OperatorApproval, Owner, TokenApproval, TokenUri};
    use dojo_erc::erc721::systems::{
        erc721_approve, erc721_set_approval_for_all, erc721_transfer_from
    };

    #[storage]
    struct Storage {
        world: IWorldDispatcher,
        _owner: ContractAddress,
        _name: felt252,
        _symbol: felt252,
        _token_uri: felt252,
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
        ref self: ContractState,
        world: IWorldDispatcher,
        owner: ContractAddress,
        name: felt252,
        symbol: felt252,
        uri: felt252,
    ) {
        self.world.write(world);
        self._owner.write(owner);
        self._name.write(name);
        self._symbol.write(symbol);
        self._token_uri.write(uri);
    }

    #[external(v0)]
    fn owner(self: @ContractState) -> ContractAddress {
        self._owner.read()
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
    fn token_uri(self: @ContractState, token_id: u256) -> felt252 {
        assert(exists(self, token_id), 'invalid token_id');

        // TODO : return self.token_uri + '/' + token_id
        self._token_uri.read()
    }

    #[external(v0)]
    fn balance_of(self: @ContractState, account: ContractAddress) -> u256 {
        let token = get_contract_address();
        let balance = get!(self.world.read(), (token, account), Balance);
        balance.amount.into()
    }

    fn exists(self: @ContractState, token_id: u256) -> bool {
        owner_of(self, token_id).is_non_zero()
    }


    #[external(v0)]
    fn owner_of(self: @ContractState, token_id: u256) -> ContractAddress {
        let token = get_contract_address();
        let owner = get!(self.world.read(), (token, u256_into_felt252(token_id)), Owner);
        owner.address
    }

    #[external(v0)]
    fn get_approved(self: @ContractState, token_id: u256) -> ContractAddress {
        assert(exists(self, token_id), 'invalid token_id');

        let token = get_contract_address();
        let approved = get!(self.world.read(), (token, u256_into_felt252(token_id)), TokenApproval);
        approved.address
    }

    #[external(v0)]
    fn is_approved_for_all(
        self: @ContractState, owner: ContractAddress, operator: ContractAddress
    ) -> bool {
        let token = get_contract_address();
        let result = get!(self.world.read(), (token, owner, operator), OperatorApproval);
        result.approved
    }

    #[external(v0)]
    fn approve(ref self: ContractState, to: ContractAddress, token_id: u256) {
        assert(exists(@self, token_id), 'invalid token_id');

        let token = get_contract_address();
        let caller = get_caller_address();

        let mut calldata = ArrayTrait::new();
        calldata.append(token.into());
        calldata.append(caller.into());
        calldata.append(u256_into_felt252(token_id));
        calldata.append(to.into());
        self.world.read().execute('erc721_approve'.into(), calldata);
        let owner = owner_of(@self, token_id);
        self.emit(Approval { owner, to, token_id });
    }

    #[external(v0)]
    fn set_approval_for_all(ref self: ContractState, operator: ContractAddress, approved: bool) {

        let token = get_contract_address();
        let caller = get_caller_address();

        let mut calldata = ArrayTrait::new();
        calldata.append(token.into());
        calldata.append(caller.into());
        calldata.append(operator.into());
        calldata.append(approved.into());
        self.world.read().execute('erc721_set_approval_for_all'.into(), calldata);
        self.emit(ApprovalForAll { owner: caller, operator, approved });
    }

    #[external(v0)]
    fn transfer(ref self: ContractState, to: ContractAddress, token_id: u256) {
        transfer_from(ref self, get_caller_address(), to, token_id);
    }

    #[external(v0)]
    fn safe_transfer_from(
        ref self: ContractState, from: ContractAddress, to: ContractAddress, token_id: u256
    ) { // TODO: implement
    // TODO: revert on non existing id
    // panic(array!['not implemented'])
    }

    #[external(v0)]
    fn transfer_from(
        ref self: ContractState, from: ContractAddress, to: ContractAddress, token_id: u256
    ) {
        assert(exists(@self, token_id), 'invalid token_id');

        let token = get_contract_address();

        let mut calldata = ArrayTrait::new();
        calldata.append(token.into());
        calldata.append(from.into());
        calldata.append(to.into());
        calldata.append(u256_into_felt252(token_id));
        self.world.read().execute('erc721_transfer_from'.into(), calldata);
        self.emit(Transfer { from, to, token_id });
    }


    #[external(v0)]
    fn mint(ref self: ContractState, to: ContractAddress, token_id: u256) {
        assert(!exists(@self, token_id), 'already minted');

        let token = get_contract_address();

        let mut calldata: Array<felt252> = ArrayTrait::new();
        calldata.append(token.into());
        calldata.append(u256_into_felt252(token_id));
        calldata.append(to.into());
        self.world.read().execute('erc721_mint'.into(), calldata);
        self.emit(Transfer { from: Zeroable::zero(), to, token_id });
    }

    #[external(v0)]
    fn burn(ref self: ContractState, token_id: u256) {
        assert(exists(@self, token_id), 'invalid token_id');
        assert(owner_of(@self, token_id) == get_caller_address(), 'caller is not owner');

        let token = get_contract_address();

        let mut calldata: Array<felt252> = ArrayTrait::new();
        calldata.append(token.into());
        calldata.append(u256_into_felt252(token_id));

        self.world.read().execute('erc721_burn'.into(), calldata);
        self.emit(Transfer { from: get_caller_address(), to: Zeroable::zero(), token_id });
    }

    fn u256_into_felt252(val: u256) -> felt252 {
        // temporary, until TryInto of this is in corelib
        val.low.into() + val.high.into() * 0x100000000000000000000000000000000
    }
}
