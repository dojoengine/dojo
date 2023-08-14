#[starknet::contract]
mod ERC721 {
    use array::ArrayTrait;
    use option::OptionTrait;
    use starknet::{ContractAddress, get_caller_address, get_contract_address};
    use traits::{Into, TryInto};
    use zeroable::Zeroable;

    use dojo::world::{IWorldDispatcher, IWorldDispatcherTrait};
    use dojo_erc::erc721::components::{Balance, OperatorApproval, Owner, TokenApproval};
    use dojo_erc::erc721::interface::IERC721;

    #[storage]
    struct Storage {
        world: IWorldDispatcher,
        owner_: ContractAddress,
        name_: felt252,
        symbol_: felt252,
        token_uri_: felt252,
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
        self.owner_.write(owner);
        self.name_.write(name);
        self.symbol_.write(symbol);
        self.token_uri_.write(uri);
    }

    #[external(v0)]
    impl ERC721 of IERC721<ContractState> {
        fn owner(self: @ContractState) -> ContractAddress {
            self.owner_.read()
        }

        fn name(self: @ContractState) -> felt252 {
            self.name_.read()
        }

        fn symbol(self: @ContractState) -> felt252 {
            self.symbol_.read()
        }

        fn token_uri(self: @ContractState, token_id: u256) -> felt252 {
            // TODO : return self.token_uri + '/' + token_id
            self.token_uri_.read()
        }

        fn balance_of(self: @ContractState, account: ContractAddress) -> u256 {
            let token = get_contract_address();
            let balance = get!(self.world.read(), (token, account), Balance);
            balance.amount.into()
        }

        fn exists(self: @ContractState, token_id: u256) -> bool {
            self.owner_of(token_id).is_non_zero()
        }

        fn owner_of(self: @ContractState, token_id: u256) -> ContractAddress {
            let token = get_contract_address();
            let owner = get!(self.world.read(), (token, u256_into_felt252(token_id)), Owner);
            owner.address
        }

        fn get_approved(self: @ContractState, token_id: u256) -> ContractAddress {
            assert(self.exists(token_id), 'ERC721: invalid token_id');

            let token = get_contract_address();
            let approved = get!(
                self.world.read(), (token, u256_into_felt252(token_id)), TokenApproval
            );
            approved.address
        }

        fn is_approved_for_all(
            self: @ContractState, owner: ContractAddress, operator: ContractAddress
        ) -> bool {
            let token = get_contract_address();
            let result = get!(self.world.read(), (token, owner, operator), OperatorApproval);
            result.approved
        }

        fn approve(ref self: ContractState, to: ContractAddress, token_id: u256) {
            let token = get_contract_address();
            let caller = get_caller_address();

            let mut calldata = ArrayTrait::new();
            calldata.append(token.into());
            calldata.append(caller.into());
            calldata.append(u256_into_felt252(token_id));
            calldata.append(to.into());
            self.world.read().execute('erc721_approve'.into(), calldata);
            let owner = self.owner_of(token_id);
            self.emit(Approval { owner, to, token_id });
        }

        fn set_approval_for_all(
            ref self: ContractState, operator: ContractAddress, approved: bool
        ) {
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


        fn transfer_from(
            ref self: ContractState, from: ContractAddress, to: ContractAddress, token_id: u256
        ) {
            let token = get_contract_address();
            let caller = get_caller_address();

            let mut calldata = ArrayTrait::new();
            calldata.append(token.into());
            calldata.append(caller.into());
            calldata.append(from.into());
            calldata.append(to.into());
            calldata.append(u256_into_felt252(token_id));
            self.world.read().execute('erc721_transfer_from'.into(), calldata);
            self.emit(Transfer { from, to, token_id });
        }


        fn transfer(ref self: ContractState, to: ContractAddress, token_id: u256) {
            self.transfer_from(get_caller_address(), to, token_id);
        }

        fn mint(ref self: ContractState, to: ContractAddress, token_id: u256) {
            let token = get_contract_address();

            let mut calldata: Array<felt252> = ArrayTrait::new();
            calldata.append(token.into());
            calldata.append(u256_into_felt252(token_id));
            calldata.append(to.into());
            self.world.read().execute('erc721_mint'.into(), calldata);
            self.emit(Transfer { from: Zeroable::zero(), to, token_id });
        }

        fn burn(ref self: ContractState, token_id: u256) {
            let token = get_contract_address();
            let caller = get_caller_address();

            let mut calldata: Array<felt252> = ArrayTrait::new();
            calldata.append(token.into());
            calldata.append(caller.into());
            calldata.append(u256_into_felt252(token_id));

            self.world.read().execute('erc721_burn'.into(), calldata);
            self.emit(Transfer { from: get_caller_address(), to: Zeroable::zero(), token_id });
        }
    }

    fn u256_into_felt252(val: u256) -> felt252 {
        val.try_into().unwrap()
    }
}
