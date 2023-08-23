#[starknet::contract]
mod ERC1155 {
    use array::ArrayTrait;
    use option::OptionTrait;
    use clone::Clone;
    use array::ArrayTCloneImpl;
    use starknet::{ContractAddress, get_caller_address, get_contract_address};
    use traits::{Into, TryInto};
    use zeroable::Zeroable;
    use serde::Serde;
    use dojo::world::{IWorldDispatcher, IWorldDispatcherTrait};
    use dojo_erc::erc1155::components::{
        Uri, ERC1155BalanceTrait, OperatorApproval, OperatorApprovalTrait
    };
    use dojo_erc::erc1155::interface::{
        IERC1155, IERC1155Metadata, IERC1155TokenReceiver, IERC1155TokenReceiverDispatcher,
        IERC1155TokenReceiverDispatcherTrait, IERC1155_ID, IERC1155_METADATA_ID,
        IERC1155_RECEIVER_ID
    };
    use dojo_erc::erc165::interface::{IERC165, IERC165_ID};

    use dojo_erc::erc_common::utils::{to_calldata, ToCallDataTrait, system_calldata};

    use dojo_erc::erc1155::systems::{
        ERC1155SetApprovalForAllParams, ERC1155SafeTransferFromParams,
        ERC1155SafeBatchTransferFromParams, ERC1155MintParams, ERC1155BurnParams
    };

    const UNLIMITED_ALLOWANCE: felt252 =
        3618502788666131213697322783095070105623107215331596699973092056135872020480;


    #[derive(Clone, Drop, Serde, starknet::Event)]
    struct TransferSingle {
        operator: ContractAddress,
        from: ContractAddress,
        to: ContractAddress,
        id: u256,
        value: u256
    }

    #[derive(Clone, Drop, Serde, starknet::Event)]
    struct TransferBatch {
        operator: ContractAddress,
        from: ContractAddress,
        to: ContractAddress,
        ids: Array<u256>,
        values: Array<u256>
    }

    #[derive(Clone, Drop, Serde, starknet::Event)]
    struct ApprovalForAll {
        owner: ContractAddress,
        operator: ContractAddress,
        approved: bool
    }

    #[starknet::interface]
    trait IERC1155Events<ContractState> {
        fn on_transfer_single(ref self: ContractState, event: TransferSingle);
        fn on_transfer_batch(ref self: ContractState, event: TransferBatch);
        fn on_approval_for_all(ref self: ContractState, event: ApprovalForAll);
    }

    #[event]
    #[derive(Drop, starknet::Event)]
    enum Event {
        TransferSingle: TransferSingle,
        TransferBatch: TransferBatch,
        ApprovalForAll: ApprovalForAll
    }

    #[storage]
    struct Storage {
        world: IWorldDispatcher,
        owner_: ContractAddress
    }

    //
    // Constructor
    //

    #[constructor]
    fn constructor(
        ref self: ContractState, world: ContractAddress, deployer: ContractAddress, uri: felt252
    ) {
        self.world.write(IWorldDispatcher { contract_address: world });
        self.owner_.write(deployer);
        self
            .world
            .read()
            .execute('ERC1155SetUri', to_calldata(get_contract_address()).plus(uri).data);
    }

    #[external(v0)]
    impl ERC165 of IERC165<ContractState> {
        fn supports_interface(self: @ContractState, interface_id: u32) -> bool {
            interface_id == IERC165_ID
                || interface_id == IERC1155_ID
                || interface_id == IERC1155_METADATA_ID
        }
    }

    #[external(v0)]
    impl ERC1155Metadata of IERC1155Metadata<ContractState> {
        fn uri(self: @ContractState, token_id: u256) -> felt252 {
            let token = get_contract_address();
            let token_id_felt: felt252 = token_id.try_into().unwrap();
            get!(self.world.read(), (token), Uri).uri
        }
    }


    #[external(v0)]
    impl ERC1155 of IERC1155<ContractState> {
        fn balance_of(self: @ContractState, account: ContractAddress, id: u256) -> u256 {
            ERC1155BalanceTrait::balance_of(
                self.world.read(), get_contract_address(), account, id.try_into().unwrap()
            )
                .into()
        }

        fn balance_of_batch(
            self: @ContractState, accounts: Array<ContractAddress>, ids: Array<u256>
        ) -> Array<u256> {
            assert(ids.len() == accounts.len(), 'ERC1155: invalid length');

            let mut batch_balances = ArrayTrait::new();
            let mut index = 0;
            loop {
                if index == ids.len() {
                    break batch_balances.clone();
                }
                batch_balances
                    .append(
                        ERC1155BalanceTrait::balance_of(
                            self.world.read(),
                            get_contract_address(),
                            *accounts.at(index),
                            (*ids.at(index)).try_into().unwrap()
                        )
                            .into()
                    );
                index += 1;
            }
        }

        fn is_approved_for_all(
            self: @ContractState, account: ContractAddress, operator: ContractAddress
        ) -> bool {
            OperatorApprovalTrait::is_approved_for_all(
                self.world.read(), get_contract_address(), account, operator
            )
        }

        fn set_approval_for_all(
            ref self: ContractState, operator: ContractAddress, approved: bool
        ) {
            self
                .world
                .read()
                .execute(
                    'ERC1155SetApprovalForAll',
                    system_calldata(
                        ERC1155SetApprovalForAllParams {
                            token: get_contract_address(),
                            owner: get_caller_address(),
                            operator,
                            approved
                        }
                    )
                );
        }

        fn safe_transfer_from(
            ref self: ContractState,
            from: ContractAddress,
            to: ContractAddress,
            id: u256,
            amount: u256,
            data: Array<u8>
        ) {
            self
                .world
                .read()
                .execute(
                    'ERC1155SafeTransferFrom',
                    system_calldata(
                        ERC1155SafeTransferFromParams {
                            token: get_contract_address(),
                            operator: get_caller_address(),
                            from,
                            to,
                            id: id.try_into().unwrap(),
                            amount: amount.try_into().unwrap(),
                            data: data
                        }
                    )
                );
        }

        fn safe_batch_transfer_from(
            ref self: ContractState,
            from: ContractAddress,
            to: ContractAddress,
            mut ids: Array<u256>,
            mut amounts: Array<u256>,
            data: Array<u8>
        ) {
            let mut idsf: Array<felt252> = ArrayTrait::new();
            let mut amounts128: Array<u128> = ArrayTrait::new();
            loop {
                if ids.len() == 0 {
                    break;
                }
                idsf.append(ids.pop_front().unwrap().try_into().unwrap());
                amounts128.append(amounts.pop_front().unwrap().try_into().unwrap());
            };

            self
                .world
                .read()
                .execute(
                    'ERC1155SafeBatchTransferFrom',
                    system_calldata(
                        ERC1155SafeBatchTransferFromParams {
                            token: get_contract_address(),
                            operator: get_caller_address(),
                            from,
                            to,
                            ids: idsf,
                            amounts: amounts128,
                            data: data
                        }
                    )
                );
        }
    }

    #[external(v0)]
    impl ERC1155Events of IERC1155Events<ContractState> {
        fn on_transfer_single(ref self: ContractState, event: TransferSingle) {
            assert(get_caller_address() == self.world.read().executor(), 'ERC1155: not authorized');
            self.emit(event);
        }
        fn on_transfer_batch(ref self: ContractState, event: TransferBatch) {
            assert(get_caller_address() == self.world.read().executor(), 'ERC1155: not authorized');
            self.emit(event);
        }
        fn on_approval_for_all(ref self: ContractState, event: ApprovalForAll) {
            assert(get_caller_address() == self.world.read().executor(), 'ERC1155: not authorized');
            self.emit(event);
        }
    }


    #[external(v0)]
    #[generate_trait]
    impl ERC721Custom of ERC721CustomTrait {
        // TODO: use systems directly for these instead.
        fn owner(self: @ContractState) -> ContractAddress {
            self.owner_.read()
        }

        fn mint(
            ref self: ContractState, to: ContractAddress, id: felt252, amount: u128, data: Array<u8>
        ) {
            self
                .world
                .read()
                .execute(
                    'ERC1155Mint',
                    system_calldata(
                        ERC1155MintParams {
                            token: get_contract_address(),
                            operator: get_caller_address(),
                            to,
                            ids: array![id],
                            amounts: array![amount],
                            data: data
                        }
                    )
                );
        }

        fn burn(ref self: ContractState, from: ContractAddress, id: felt252, amount: u128) {
            self
                .world
                .read()
                .execute(
                    'ERC1155Burn',
                    system_calldata(
                        ERC1155BurnParams {
                            token: get_contract_address(),
                            operator: get_caller_address(),
                            from,
                            ids: array![id],
                            amounts: array![amount]
                        }
                    )
                );
        }
    }
}
