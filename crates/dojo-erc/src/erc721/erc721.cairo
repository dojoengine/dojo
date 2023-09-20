#[starknet::contract]
mod ERC721 {
    use array::ArrayTrait;
    use option::OptionTrait;
    use starknet::{ContractAddress, get_caller_address, get_contract_address};
    use traits::{Into, TryInto};
    use zeroable::Zeroable;
    use serde::Serde;
    use clone::Clone;

    use dojo::world::{IWorldDispatcher, IWorldDispatcherTrait};
    use dojo_erc::erc721::components::{
        ERC721Owner, ERC721OwnerTrait, BaseUri, BaseUriTrait, ERC721Balance, ERC721BalanceTrait,
        ERC721TokenApproval, ERC721TokenApprovalTrait, OperatorApproval, OperatorApprovalTrait
    };
    use dojo_erc::erc721::systems::{
        ERC721ApproveParams, ERC721SetApprovalForAllParams, ERC721TransferFromParams,
        ERC721MintParams, ERC721BurnParams
    };

    use dojo_erc::erc165::interface::{IERC165, IERC165_ID};
    use dojo_erc::erc721::interface::{
        IERC721, IERC721Metadata, IERC721Custom, IERC721_ID, IERC721_METADATA_ID
    };

    use dojo_erc::erc_common::utils::{to_calldata, ToCallDataTrait, system_calldata};


    #[storage]
    struct Storage {
        world: IWorldDispatcher,
        owner_: ContractAddress, // TODO: move in components
        name_: felt252,
        symbol_: felt252,
    }

    #[derive(Clone, Drop, Serde, PartialEq, starknet::Event)]
    struct Transfer {
        from: ContractAddress,
        to: ContractAddress,
        token_id: u256
    }

    #[derive(Clone, Drop, Serde, PartialEq, starknet::Event)]
    struct Approval {
        owner: ContractAddress,
        to: ContractAddress,
        token_id: u256
    }

    #[derive(Clone, Drop, Serde, PartialEq, starknet::Event)]
    struct ApprovalForAll {
        owner: ContractAddress,
        operator: ContractAddress,
        approved: bool
    }

    #[event]
    #[derive(Drop, PartialEq, starknet::Event)]
    enum Event {
        Transfer: Transfer,
        Approval: Approval,
        ApprovalForAll: ApprovalForAll
    }

    #[starknet::interface]
    trait IERC721Events<ContractState> {
        fn on_transfer(ref self: ContractState, event: Transfer);
        fn on_approval(ref self: ContractState, event: Approval);
        fn on_approval_for_all(ref self: ContractState, event: ApprovalForAll);
    }

    //
    // Constructor
    //

    #[constructor]
    fn constructor(
        ref self: ContractState, owner: ContractAddress, name: felt252, symbol: felt252,
    ) {
        self.owner_.write(owner);
        self.name_.write(name);
        self.symbol_.write(symbol);
    }

    #[external(v0)]
    impl ERC165 of IERC165<ContractState> {
        fn supports_interface(self: @ContractState, interface_id: u32) -> bool {
            interface_id == IERC165_ID
                || interface_id == IERC721_ID
                || interface_id == IERC721_METADATA_ID
        }
    }

    #[external(v0)]
    impl ERC721 of IERC721<ContractState> {
        fn balance_of(self: @ContractState, account: ContractAddress) -> u256 {
            ERC721BalanceTrait::balance_of(self.world.read(), get_contract_address(), account)
                .into()
        }

        fn owner_of(self: @ContractState, token_id: u256) -> ContractAddress {
            let owner = ERC721OwnerTrait::owner_of(
                self.world.read(), get_contract_address(), token_id.try_into().unwrap()
            );
            assert(owner.is_non_zero(), 'ERC721: invalid token_id');
            owner
        }

        fn get_approved(self: @ContractState, token_id: u256) -> ContractAddress {
            assert(self.exists(token_id), 'ERC721: invalid token_id');

            let token_id_felt: felt252 = token_id.try_into().unwrap();
            ERC721TokenApprovalTrait::get_approved(
                self.world.read(), get_contract_address(), token_id_felt
            )
        }

        fn approve(ref self: ContractState, to: ContractAddress, token_id: u256) {
            let token = get_contract_address();
            let caller = get_caller_address();
            let token_id: felt252 = token_id.try_into().unwrap();
            let world = self.world.read();
            assert(caller != to, 'ERC721: invalid self approval');

            let owner = ERC721OwnerTrait::owner_of(world, token, token_id);
            assert(owner.is_non_zero(), 'ERC721: invalid token_id');

            let is_approved_for_all = OperatorApprovalTrait::is_approved_for_all(
                world, token, owner, caller
            );
            // // ERC721: approve caller is not token owner or approved for all 
            assert(caller == owner || is_approved_for_all, 'ERC721: unauthorized caller');
            ERC721TokenApprovalTrait::unchecked_approve(world, token, token_id, to,);

            // emit events
            let event = Approval { owner, to, token_id: token_id.into() };
            IERC721EventsDispatcher { contract_address: token }.on_approval(event.clone());
            emit!(world, event);
        }

        fn is_approved_for_all(
            self: @ContractState, owner: ContractAddress, operator: ContractAddress
        ) -> bool {
            OperatorApprovalTrait::is_approved_for_all(
                self.world.read(), get_contract_address(), owner, operator
            );
            let approval = get!(
                self.world.read(), (get_contract_address(), owner, operator), OperatorApproval
            );
            approval.approved
        }

        fn set_approval_for_all(
            ref self: ContractState, operator: ContractAddress, approved: bool
        ) {
            self
                .world
                .read()
                .execute(
                    'ERC721SetApprovalForAll',
                    system_calldata(
                        ERC721SetApprovalForAllParams {
                            token: get_contract_address(),
                            owner: get_caller_address(),
                            operator,
                            approved
                        }
                    )
                );
        }

        fn transfer_from(
            ref self: ContractState, from: ContractAddress, to: ContractAddress, token_id: u256
        ) {
            self
                .world
                .read()
                .execute(
                    'ERC721TransferFrom',
                    system_calldata(
                        ERC721TransferFromParams {
                            token: get_contract_address(),
                            caller: get_caller_address(),
                            from,
                            to,
                            token_id: token_id.try_into().unwrap()
                        }
                    )
                );
        }

        fn safe_transfer_from(
            ref self: ContractState,
            from: ContractAddress,
            to: ContractAddress,
            token_id: u256,
            data: Span<felt252>
        ) {
            // TODO: check if we should do it
            panic(array!['not implemented !']);
        }
    }

    #[external(v0)]
    impl ERC721Metadata of IERC721Metadata<ContractState> {
        fn name(self: @ContractState) -> felt252 {
            self.name_.read()
        }

        fn symbol(self: @ContractState) -> felt252 {
            self.symbol_.read()
        }

        fn token_uri(self: @ContractState, token_id: u256) -> felt252 {
            // TODO : add token_id to base_uri
            assert(self.exists(token_id), 'ERC721: invalid token_id');
            BaseUriTrait::get_base_uri(self.world.read(), get_contract_address())
        }
    }


    #[external(v0)]
    impl ERC721Custom of IERC721Custom<ContractState> {
        /// Should be called after the contract is added as the system
        /// and has write access to ERC components on the world
        fn init_world(ref self: ContractState, world: ContractAddress, uri: felt252) {
            if 0.try_into().unwrap() == self.world.read().contract_address {
                self.world.write(IWorldDispatcher { contract_address: world });
                BaseUriTrait::unchecked_set_base_uri(
                    self.world.read(), get_contract_address(), uri
                );
            }
        }

        fn exists(self: @ContractState, token_id: u256) -> bool {
            self.owner_of(token_id).is_non_zero()
        }

        fn owner(self: @ContractState) -> ContractAddress {
            self.owner_.read()
        }

        fn transfer(ref self: ContractState, to: ContractAddress, token_id: u256) {
            self.transfer_from(get_caller_address(), to, token_id);
        }

        fn mint(ref self: ContractState, to: ContractAddress, token_id: u256) {
            self
                .world
                .read()
                .execute(
                    'ERC721Mint',
                    system_calldata(
                        ERC721MintParams {
                            token: get_contract_address(),
                            recipient: to,
                            token_id: token_id.try_into().unwrap()
                        }
                    )
                );
        }

        fn burn(ref self: ContractState, token_id: u256) {
            self
                .world
                .read()
                .execute(
                    'ERC721Burn',
                    system_calldata(
                        ERC721BurnParams {
                            token: get_contract_address(),
                            caller: get_caller_address(),
                            token_id: token_id.try_into().unwrap()
                        }
                    )
                );
        }
    }


    #[external(v0)]
    impl ERC721EventEmitter of IERC721Events<ContractState> {
        fn on_transfer(ref self: ContractState, event: Transfer) {
            assert(get_caller_address() == self.world.read().executor(), 'ERC721: not authorized');
            self.emit(event);
        }
        fn on_approval(ref self: ContractState, event: Approval) {
            assert(get_caller_address() == self.world.read().executor(), 'ERC721: not authorized');
            self.emit(event);
        }
        fn on_approval_for_all(ref self: ContractState, event: ApprovalForAll) {
            assert(get_caller_address() == self.world.read().executor(), 'ERC721: not authorized');
            self.emit(event);
        }
    }
}
