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
        token_name: felt252,
        token_symbol: felt252,
        token_uri: felt252,
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
        ref self: ContractState, world: IWorldDispatcher, name: felt252, symbol: felt252
    ) {
        self.world.write(world);
        self.token_name.write(name);
        self.token_symbol.write(symbol);
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
    fn uri(self: @ContractState) -> felt252 {
        self.token_uri.read()
    }

    #[external(v0)]
    fn balance_of(self: @ContractState, account: ContractAddress) -> u256 {
        let token = get_contract_address();
        let balance = get !(self.world.read(), (token, account), Balance);
        balance.amount.into()
    }

    #[external(v0)]
    fn owner_of(self: @ContractState, token_id: u256) -> ContractAddress {
        let token = get_contract_address();
        let owner = get !(self.world.read(), (token, u256_into_felt252(token_id)), Owner);
        owner.address
    }

    #[external(v0)]
    fn get_approved(self: @ContractState, token_id: u256) -> ContractAddress {
        let token = get_contract_address();
        let approved = get !(
            self.world.read(), (token, u256_into_felt252(token_id)), TokenApproval
        );
        approved.address
    }

    #[external(v0)]
    fn is_approved_for_all(
        self: @ContractState, owner: ContractAddress, operator: ContractAddress
    ) -> bool {
        let token = get_contract_address();
        let result = get !(self.world.read(), (token, owner, operator), OperatorApproval);
        result.approved
    }

    #[external(v0)]
    fn approve(ref self: ContractState, to: ContractAddress, token_id: u256) {
        let mut calldata = ArrayTrait::new();
        let owner = owner_of(@self, token_id);
        calldata.append(u256_into_felt252(token_id));
        calldata.append(to.into());
        self.world.read().execute('erc721_approve'.into(), calldata.span());
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
        calldata.append(approved.into());
        self.world.read().execute('erc721_set_approval_for_all'.into(), calldata.span());
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
        self.world.read().execute('erc721_transfer_from'.into(), calldata.span());
        self.emit(Transfer { from, to, token_id });
    }

    fn u256_into_felt252(val: u256) -> felt252 {
        // temporary, until TryInto of this is in corelib
        val.low.into() + val.high.into() * 0x100000000000000000000000000000000
    }
}
