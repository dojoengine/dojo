#[starknet::contract]
mod ERC1155 {
    use array::ArrayTrait;
    use option::OptionTrait;
    use clone::Clone;
    use array::ArrayTCloneImpl;
    use starknet::{ContractAddress, get_caller_address, get_contract_address};
    use traits::{Into, TryInto};
    use zeroable::Zeroable;
    use dojo::world::{IWorldDispatcher, IWorldDispatcherTrait};
    use dojo_erc::erc1155::components::{OperatorApproval, Uri, Balance};
    use dojo_erc::erc1155::interface::IERC1155;

    const UNLIMITED_ALLOWANCE: felt252 =
        3618502788666131213697322783095070105623107215331596699973092056135872020480;

    // Account
    const IACCOUNT_ID: u32 = 0xa66bd575_u32;
    // ERC 165 interface codes
    const INTERFACE_ERC165: u32 = 0x01ffc9a7_u32;
    const INTERFACE_ERC1155: u32 = 0xd9b67a26_u32;
    const INTERFACE_ERC1155_METADATA: u32 = 0x0e89341c_u32;
    const INTERFACE_ERC1155_RECEIVER: u32 = 0x4e2312e0_u32;
    const ON_ERC1155_RECEIVED_SELECTOR: u32 = 0xf23a6e61_u32;
    const ON_ERC1155_BATCH_RECEIVED_SELECTOR: u32 = 0xbc197c81_u32;

    #[starknet::interface]
    trait IERC1155TokenReceiver {
        fn on_erc1155_received(
            self: ContractState,
            operator: ContractAddress,
            from: ContractAddress,
            token_id: u256,
            amount: u256,
            data: Array<u8>
        ) -> u32;
        fn on_erc1155_batch_received(
            self: ContractState,
            operator: ContractAddress,
            from: ContractAddress,
            token_ids: Array<u256>,
            amounts: Array<u256>,
            data: Array<u8>
        ) -> u32;
    }

    #[starknet::interface]
    trait IERC165 {
        fn supports_interface(self: ContractState, interface_id: u32) -> bool;
    }

    #[event]
    #[derive(Drop, starknet::Event)]
    enum Event {
        TransferSingle: TransferSingle,
        TransferBatch: TransferBatch,
        ApprovalForAll: ApprovalForAll
    }

    #[derive(Drop, starknet::Event)]
    struct TransferSingle {
        operator: ContractAddress,
        from: ContractAddress,
        to: ContractAddress,
        id: u256,
        value: u256
    }

    #[derive(Drop, starknet::Event)]
    struct TransferBatch {
        operator: ContractAddress,
        from: ContractAddress,
        to: ContractAddress,
        ids: Array<u256>,
        values: Array<u256>
    }

    #[derive(Drop, starknet::Event)]
    struct ApprovalForAll {
        owner: ContractAddress,
        operator: ContractAddress,
        approved: bool
    }

    #[storage]
    struct Storage {
        world: IWorldDispatcher, 
    }

    //
    // Constructor
    //

    #[constructor]
    fn constructor(ref self: ContractState, world: ContractAddress, uri: felt252) {
        self.world.write(IWorldDispatcher { contract_address: world });
        self._set_uri(uri);
    }

    #[external(v0)]
    impl ERC1155 of IERC1155<ContractState> {
        fn supports_interface(self: @ContractState, interface_id: u32) -> bool {
            interface_id == INTERFACE_ERC165
                || interface_id == INTERFACE_ERC1155
                || interface_id == INTERFACE_ERC1155_METADATA
        }

        //
        // ERC1155Metadata
        //
        fn uri(self: @ContractState, token_id: u256) -> felt252 {
            let token = get_contract_address();
            let token_id_felt: felt252 = token_id.try_into().unwrap();
            get!(self.world.read(), (token, token_id_felt), Uri).uri
        }

        //
        // ERC1155
        //
        fn balance_of(self: @ContractState, account: ContractAddress, id: u256) -> u256 {
            self._balance_of(account, id)
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
                batch_balances.append(self._balance_of(*accounts.at(index), *ids.at(index)));
                index += 1;
            }
        }

        fn is_approved_for_all(
            self: @ContractState, account: ContractAddress, operator: ContractAddress
        ) -> bool {
            self._is_approved_for_all(account, operator)
        }

        fn set_approval_for_all(
            ref self: ContractState, operator: ContractAddress, approved: bool
        ) {
            let caller = get_caller_address();
            self._set_approval_for_all(caller, operator, approved);
        }

        fn safe_transfer_from(
            ref self: ContractState,
            from: ContractAddress,
            to: ContractAddress,
            id: u256,
            amount: u256,
            data: Array<u8>
        ) {
            let caller = get_caller_address();
            assert(
                caller == from || self._is_approved_for_all(from, caller),
                'ERC1155: insufficient approval'
            );
            self._safe_transfer_from(from, to, id, amount, data);
        }

        fn safe_batch_transfer_from(
            ref self: ContractState,
            from: ContractAddress,
            to: ContractAddress,
            ids: Array<u256>,
            amounts: Array<u256>,
            data: Array<u8>
        ) {
            let caller = get_caller_address();
            assert(
                caller == from || self._is_approved_for_all(from, caller),
                'ERC1155: insufficient approval'
            );
            self._safe_batch_transfer_from(from, to, ids, amounts, data);
        }
    }

    //
    // Internal
    //

    #[generate_trait]
    impl PrivateFunctions of PrivateFunctionsTrait {
        fn _balance_of(self: @ContractState, account: ContractAddress, id: u256) -> u256 {
            let token = get_contract_address();
            let id_felt: felt252 = id.try_into().unwrap();
            get!(self.world.read(), (token, account, id_felt), Balance).amount.into()
        }

        fn _is_approved_for_all(
            self: @ContractState, account: ContractAddress, operator: ContractAddress
        ) -> bool {
            let token = get_contract_address();
            get!(self.world.read(), (token, account, operator), OperatorApproval).approved
        }

        fn _update(
            ref self: ContractState,
            from: ContractAddress,
            to: ContractAddress,
            ids: Array<u256>,
            amounts: Array<u256>,
            data: Array<u8>
        ) {
            assert(ids.len() == amounts.len(), 'ERC1155: invalid length');

            let operator = get_caller_address();
            let token = get_contract_address();
            let mut calldata = ArrayTrait::new();
            calldata.append(token.into());
            calldata.append(operator.into());
            calldata.append(from.into());
            calldata.append(to.into());
            calldata.append(ids.len().into());

            // cloning because loop takes ownership
            let ids_clone = ids.clone();
            let amounts_clone = ids.clone();
            let data_clone = data.clone();

            let mut index = 0;
            loop {
                if index == ids.len() {
                    break ();
                }
                let id: felt252 = (*ids.at(index)).try_into().unwrap();
                calldata.append(id);
                index += 1;
            };
            calldata.append(amounts.len().into());
            let mut index = 0;
            loop {
                if index == amounts.len() {
                    break ();
                }
                let amount: felt252 = (*amounts.at(index)).try_into().unwrap();
                calldata.append(amount);
                index += 1;
            };
            calldata.append(data.len().into());
            let mut index = 0;
            loop {
                if index == data.len() {
                    break ();
                }
                let data_cell: felt252 = (*data.at(index)).into();
                calldata.append(data_cell);
                index += 1;
            };
            self.world.read().execute('ERC1155Update'.into(), calldata);

            if (ids_clone.len() == 1) {
                let id = *ids_clone.at(0);
                let amount = *amounts_clone.at(0);

                self.emit(TransferSingle { operator, from, to, id, value: amount });

                if (to.is_non_zero()) {
                    _do_safe_transfer_acceptance_check(operator, from, to, id, amount, data_clone);
                } else {
                    self
                        .emit(
                            TransferBatch {
                                operator: operator,
                                from: from,
                                to: to,
                                ids: ids_clone.clone(),
                                values: amounts_clone.clone()
                            }
                        );
                    if (to.is_non_zero()) {
                        _do_safe_batch_transfer_acceptance_check(
                            operator, from, to, ids_clone, amounts_clone, data_clone
                        );
                    }
                }
            }
        }

        fn _safe_transfer_from(
            ref self: ContractState,
            from: ContractAddress,
            to: ContractAddress,
            id: u256,
            amount: u256,
            data: Array<u8>
        ) {
            assert(to.is_non_zero(), 'ERC1155: invalid receiver');
            assert(from.is_non_zero(), 'ERC1155: invalid sender');

            let ids = _as_singleton_array(id);
            let amounts = _as_singleton_array(amount);
            self._update(from, to, ids, amounts, data);
        }

        fn _safe_batch_transfer_from(
            ref self: ContractState,
            from: ContractAddress,
            to: ContractAddress,
            ids: Array<u256>,
            amounts: Array<u256>,
            data: Array<u8>
        ) {
            assert(to.is_non_zero(), 'ERC1155: invalid receiver');
            assert(from.is_non_zero(), 'ERC1155: invalid sender');
            self._update(from, to, ids, amounts, data);
        }

        fn _set_uri(ref self: ContractState, uri: felt252) {
            let token = get_contract_address();
            let mut calldata = ArrayTrait::new();
            calldata.append(token.into());
            calldata.append(uri);
            self.world.read().execute('ERC1155SetUri'.into(), calldata);
        }

        fn _mint(
            ref self: ContractState, to: ContractAddress, id: u256, amount: u256, data: Array<u8>
        ) {
            assert(to.is_non_zero(), 'ERC1155: invalid receiver');

            let ids = _as_singleton_array(id);
            let amounts = _as_singleton_array(amount);
            self._update(Zeroable::zero(), to, ids, amounts, data);
        }

        fn _mint_batch(
            ref self: ContractState,
            to: ContractAddress,
            ids: Array<u256>,
            amounts: Array<u256>,
            data: Array<u8>
        ) {
            assert(to.is_non_zero(), 'ERC1155: invalid receiver');
            self._update(Zeroable::zero(), to, ids, amounts, data)
        }

        fn _burn(
            ref self: ContractState, from: ContractAddress, id: u256, amount: u256, data: Array<u8>
        ) {
            assert(from.is_non_zero(), 'ERC1155: invalid sender');

            let ids = _as_singleton_array(id);
            let amounts = _as_singleton_array(amount);
            self._update(from, Zeroable::zero(), ids, amounts, data);
        }

        fn _burn_batch(
            ref self: ContractState,
            from: ContractAddress,
            ids: Array<u256>,
            amounts: Array<u256>,
            data: Array<u8>
        ) {
            assert(from.is_non_zero(), 'ERC1155: invalid sender');
            self._update(from, Zeroable::zero(), ids, amounts, data);
        }

        fn _set_approval_for_all(
            ref self: ContractState,
            owner: ContractAddress,
            operator: ContractAddress,
            approved: bool
        ) {
            assert(owner != operator, 'ERC1155: wrong approval');
            let token = get_contract_address();
            let mut calldata: Array<felt252> = ArrayTrait::new();
            calldata.append(token.into());
            calldata.append(owner.into());
            calldata.append(operator.into());
            if approved {
                calldata.append(1);
            } else {
                calldata.append(0);
            }
            self.world.read().execute('ERC1155SetApprovalForAll'.into(), calldata);

            self.emit(ApprovalForAll { owner, operator, approved });
        }
    }

    fn _do_safe_transfer_acceptance_check(
        operator: ContractAddress,
        from: ContractAddress,
        to: ContractAddress,
        id: u256,
        amount: u256,
        data: Array<u8>
    ) {
        if (IERC165Dispatcher {
            contract_address: to
        }.supports_interface(INTERFACE_ERC1155_RECEIVER)) {
            assert(
                IERC1155TokenReceiverDispatcher {
                    contract_address: to
                }
                    .on_erc1155_received(
                        operator, from, id, amount, data
                    ) == ON_ERC1155_RECEIVED_SELECTOR,
                'ERC1155: ERC1155Receiver reject'
            );
            return ();
        }
        assert(
            IERC165Dispatcher { contract_address: to }.supports_interface(IACCOUNT_ID),
            'Transfer to non-ERC1155Receiver'
        );
    }

    fn _do_safe_batch_transfer_acceptance_check(
        operator: ContractAddress,
        from: ContractAddress,
        to: ContractAddress,
        ids: Array<u256>,
        amounts: Array<u256>,
        data: Array<u8>
    ) {
        if (IERC165Dispatcher {
            contract_address: to
        }.supports_interface(INTERFACE_ERC1155_RECEIVER)) {
            assert(
                IERC1155TokenReceiverDispatcher {
                    contract_address: to
                }
                    .on_erc1155_batch_received(
                        operator, from, ids, amounts, data
                    ) == ON_ERC1155_BATCH_RECEIVED_SELECTOR,
                'ERC1155: ERC1155Receiver reject'
            );
            return ();
        }
        assert(
            IERC165Dispatcher { contract_address: to }.supports_interface(IACCOUNT_ID),
            'Transfer to non-ERC1155Receiver'
        );
    }

    fn _as_singleton_array(element: u256) -> Array<u256> {
        let mut array = ArrayTrait::new();
        array.append(element);
        array
    }
}
