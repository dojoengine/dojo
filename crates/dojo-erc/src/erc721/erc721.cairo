use starknet::ContractAddress;
use serde::Serde;
use clone::Clone;

#[derive(Clone, Drop, Serde, starknet::Event)]
struct Transfer {
    from: ContractAddress,
    to: ContractAddress,
    token_id: u256
}

#[derive(Clone, Drop, Serde, starknet::Event)]
struct Approval {
    owner: ContractAddress,
    to: ContractAddress,
    token_id: u256
}

#[derive(Clone, Drop, Serde, starknet::Event)]
struct ApprovalForAll {
    owner: ContractAddress,
    operator: ContractAddress,
    approved: bool
}

#[starknet::interface]
trait IERC721EventEmitter<ContractState> {
    fn on_transfer(ref self: ContractState, event: Transfer);
    fn on_approval(ref self: ContractState, event: Approval);
    fn on_approval_for_all(ref self: ContractState, event: ApprovalForAll);
}

#[starknet::contract]
mod ERC721 {
    use array::ArrayTrait;
    use option::OptionTrait;
    use starknet::{ContractAddress, get_caller_address, get_contract_address};
    use traits::{Into, TryInto};
    use zeroable::Zeroable;

    use dojo::world::{IWorldDispatcher, IWorldDispatcherTrait};
    use dojo_erc::erc721::components::{
        ERC721Owner, ERC721OwnerTrait, BaseUri, BaseUriTrait, ERC721Balance, ERC721BalanceTrait,
        ERC721TokenApproval, ERC721TokenApprovalTrait, OperatorApproval, OperatorApprovalTrait
    };
    use dojo_erc::erc721::interface::IERC721;
    use dojo_erc::erc_common::utils::{to_calldata, ToCallDataTrait};

    use super::{Transfer, Approval, ApprovalForAll};

    use debug::PrintTrait;

    #[storage]
    struct Storage {
        world: IWorldDispatcher,
        owner_: ContractAddress, // TODO: move in components
        name_: felt252,
        symbol_: felt252,
    }

    #[event]
    #[derive(Drop, starknet::Event)]
    enum Event {
        Transfer: Transfer,
        Approval: Approval,
        ApprovalForAll: ApprovalForAll
    }

    //
    // Constructor
    //

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
        let ca = get_contract_address();
    // NOT WORKING
    // TODO : check get_contract_address value in constructor
    // BaseUriTrait::set_base_uri(world, get_contract_address(), uri);
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
            // TODO : add token_id to base_uri
            BaseUriTrait::get_base_uri(self.world.read(), get_contract_address())
        }

        fn balance_of(self: @ContractState, account: ContractAddress) -> u256 {
            ERC721BalanceTrait::balance_of(self.world.read(), get_contract_address(), account)
                .into()
        }

        fn exists(self: @ContractState, token_id: u256) -> bool {
            self.owner_of(token_id).is_non_zero()
        }

        fn owner_of(self: @ContractState, token_id: u256) -> ContractAddress {
            ERC721OwnerTrait::owner_of(
                self.world.read(), get_contract_address(), token_id.try_into().unwrap()
            )
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
            let token_id_felt: felt252 = token_id.try_into().unwrap();

            self
                .world
                .read()
                .execute(
                    'ERC721Approve',
                    to_calldata(token).plus(caller).plus(token_id_felt).plus(to).data
                );
        }

        fn is_approved_for_all(
            self: @ContractState, owner: ContractAddress, operator: ContractAddress
        ) -> bool {
            OperatorApprovalTrait::is_approved_for_all(
                self.world.read(), get_contract_address(), owner, operator
            )
        }

        fn set_approval_for_all(
            ref self: ContractState, operator: ContractAddress, approved: bool
        ) {
            let owner = get_caller_address();

            assert(owner != operator, 'ERC1155: wrong approval');

            self
                .world
                .read()
                .execute(
                    'ERC721SetApprovalForAll',
                    to_calldata(get_contract_address())
                        .plus(owner)
                        .plus(operator)
                        .plus(approved)
                        .data
                );
        }


        fn transfer_from(
            ref self: ContractState, from: ContractAddress, to: ContractAddress, token_id: u256
        ) {
            let token_id_felt: felt252 = token_id.try_into().unwrap();

            self
                .world
                .read()
                .execute(
                    'ERC721TransferFrom',
                    to_calldata(get_contract_address())
                        .plus(get_caller_address())
                        .plus(from)
                        .plus(to)
                        .plus(token_id_felt)
                        .data
                );
        }


        fn transfer(ref self: ContractState, to: ContractAddress, token_id: u256) {
            self.transfer_from(get_caller_address(), to, token_id);
        }

        fn mint(ref self: ContractState, to: ContractAddress, token_id: u256) {
            let token_id_felt: felt252 = token_id.try_into().unwrap();

            self
                .world
                .read()
                .execute(
                    'ERC721Mint',
                    to_calldata(get_contract_address())
                        .plus(to)
                        .plus(token_id_felt)
                        .data
                );
        }

        fn burn(ref self: ContractState, token_id: u256) {
            let token_id_felt: felt252 = token_id.try_into().unwrap();

            self
                .world
                .read()
                .execute(
                    'ERC721Burn',
                    to_calldata(get_contract_address())
                        .plus(get_caller_address())
                        .plus(token_id_felt)
                        .data
                );
        }
    }
}
