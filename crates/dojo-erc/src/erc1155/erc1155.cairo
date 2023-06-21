#[contract]
mod ERC1155 {
    use array::ArrayTrait;
    use option::OptionTrait;
    use clone::Clone;
    use array::ArrayTCloneImpl;
    use starknet::{ contract_address, ContractAddress, get_caller_address, get_contract_address };
    use traits::{ Into, TryInto };
    use zeroable::Zeroable;
    use dojo_core::storage::query::{
        Query,
        IntoPartitioned,
        IntoPartitionedQuery
    };
    use dojo_core::interfaces::{ IWorldDispatcher, IWorldDispatcherTrait };
    use dojo_erc::erc1155::components::{ Balance, OperatorApproval, Uri };

    const UNLIMITED_ALLOWANCE: felt252 = 3618502788666131213697322783095070105623107215331596699973092056135872020480;


    // Account
    const IACCOUNT_ID: u32 = 0xa66bd575_u32;
    // ERC 165 interface codes
    const INTERFACE_ERC165: u32 = 0x01ffc9a7_u32;
    const INTERFACE_ERC1155: u32 = 0xd9b67a26_u32;
    const INTERFACE_ERC1155_METADATA: u32 = 0x0e89341c_u32;
    const INTERFACE_ERC1155_RECEIVER: u32 = 0x4e2312e0_u32;
    const ON_ERC1155_RECEIVED_SELECTOR: u32 = 0xf23a6e61_u32;
    const ON_ERC1155_BATCH_RECEIVED_SELECTOR: u32 = 0xbc197c81_u32;

    #[abi]
    trait IERC1155TokenReceiver {
        fn on_erc1155_received(
            operator: ContractAddress,
            from: ContractAddress,
            token_id: u256,
            amount: u256,
            data: Array<u8>
        ) -> u32;
        fn on_erc1155_batch_received(
            operator: ContractAddress,
            from: ContractAddress,
            token_ids: Array<u256>,
            amounts: Array<u256>,
            data: Array<u8>
        ) -> u32;
    }

    #[abi]
    trait IERC165 {
        fn supports_interface(interface_id: u32) -> bool;
    }

    #[event]
    fn TransferSingle(operator: ContractAddress, from: ContractAddress, to: ContractAddress, id: u256, value: u256) {}

    #[event]
    fn TransferBatch(
        operator: ContractAddress,
        from: ContractAddress,
        to: ContractAddress,
        ids: Array<u256>,
        values: Array<u256>
    ) {}

    #[event]
    fn ApprovalForAll(account: ContractAddress, operator: ContractAddress, approved: bool) {}

    struct Storage {
        world_address: ContractAddress,
    }

    //
    // Constructor
    //

    #[constructor]
    fn constructor(world_address: ContractAddress, uri: felt252) {
        world_address::write(world_address);
        _set_uri(uri);
    }

    #[view]
    fn supports_interface(interface_id: u32) -> bool {
        interface_id == INTERFACE_ERC165 |
        interface_id == INTERFACE_ERC1155 |
        interface_id == INTERFACE_ERC1155_METADATA
    }

    //
    // ERC1155Metadata
    //

    #[view]
    fn uri(token_id: u256) -> felt252 {
        let token = get_contract_address();
        let query: Query = (token, (u256_as_allowance(token_id))).into_partitioned();
        let mut uri_raw = world().entity('Uri'.into(), query, 0, 0);
        let _uri = serde::Serde::<Uri>::deserialize(ref uri_raw).unwrap();
        _uri.uri
    }

    //
    // ERC1155
    //

    #[view]
    fn balance_of(account: ContractAddress, id: u256) -> u256 {
        let token = get_contract_address();
        let query: Query = (token, (account, u256_as_allowance(id))).into_partitioned();
        let mut balance_raw = world().entity('Balance'.into(), query, 0, 0);     
        let balance = serde::Serde::<Balance>::deserialize(ref balance_raw).unwrap();
        balance.amount.into()
    }

    #[view]
    fn balance_of_batch(accounts: Array<ContractAddress>, ids: Array<u256>) -> Array<u256> {
        assert(ids.len() == accounts.len(), 'ERC1155: invalid length');

        let mut batch_balances = ArrayTrait::new();
        let mut index = 0;
        loop {
            if index == ids.len() {
                break();
            }
            batch_balances.append(balance_of(*accounts.at(index), *ids.at(index)));
        };
        batch_balances
    }

    

    #[view]
    fn is_approved_for_all(account: ContractAddress, operator: ContractAddress) -> bool {
        let token = get_contract_address();
        let query: Query = (token, (account, operator)).into_partitioned();
        let mut approval_raw = world().entity('OperatorApproval'.into(), query, 0, 0);
        serde::Serde::<OperatorApproval>::deserialize(ref approval_raw).unwrap().value
    }

    #[external]
    fn set_approval_for_all(operator: ContractAddress, approved: bool) {
        let caller = get_caller_address();
        _set_approval_for_all(caller, operator, approved);
    }

    #[external]
    fn safe_transfer_from(
        from: ContractAddress,
        to: ContractAddress,
        id: u256,
        amount: u256,
        data: Array<u8>
    ) {
        let caller = get_caller_address();
        assert(caller == from | is_approved_for_all(from, caller),
            'ERC1155: insufficient approval'
        );
        _safe_transfer_from(from, to, id, amount, data);
    }

    #[external]
    fn safe_batch_transfer_from(
        from: ContractAddress,
        to: ContractAddress,
        ids: Array<u256>,
        amounts: Array<u256>,
        data: Array<u8>
    ) {
        let caller = get_caller_address();
        assert(caller == from | is_approved_for_all(from, caller),
            'ERC1155: insufficient approval'
        );
        _safe_batch_transfer_from(from, to, ids, amounts, data);
    }

    //
    // Internal
    //

    // NOTE: temporary, until we have inline commands outside of systems
    fn world() -> IWorldDispatcher {
        IWorldDispatcher { contract_address: world_address::read() }
    }

    fn _update(
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

        // cloning becasue loop takes ownership
        let ids_clone = ids.clone();
        let amounts_clone = ids.clone();
        let data_clone = data.clone();

        let mut index = 0;
        loop {
            if index == ids.len() {
                break();
            }
            let id: felt252 = u256_as_allowance(*ids.at(index));
            calldata.append(id);
            index+=1;
        };
        calldata.append(amounts.len().into());
        let mut index = 0;
        loop {
            if index == amounts.len() {
                break();
            }
            let amount: felt252 = u256_as_allowance(*amounts.at(index));
            calldata.append(amount);
            index+=1;
        };
        calldata.append(data.len().into());
        let mut index = 0;
        loop {
            if index == data.len() {
                break();
            }
            let data_cell: felt252 = (*data.at(index)).into();
            calldata.append(data_cell);
            index += 1;
        };
        world().execute('ERC1155Update'.into(), calldata.span());

        if (ids_clone.len() == 1) {
            let id = *ids_clone.at(0);
            let amount = *amounts_clone.at(0);

            TransferSingle(operator, from, to, id, amount);

            if (to.is_non_zero()) {
                _do_safe_transfer_acceptance_check(
                    operator,
                    from,
                    to,
                    id,
                    amount,
                    data_clone
                );
            } else {
                TransferBatch(operator, from, to, ids_clone.clone(), amounts_clone.clone());
                if (to.is_non_zero()) {
                    _do_safe_batch_transfer_acceptance_check(
                        operator,
                        from,
                        to,
                        ids_clone,
                        amounts_clone,
                        data_clone
                    );
                }
            }
        }
    }

    fn _safe_transfer_from(
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
        _update(from, to, ids, amounts, data);
    }

    fn _safe_batch_transfer_from(
        from: ContractAddress,
        to: ContractAddress,
        ids: Array<u256>,
        amounts: Array<u256>,
        data: Array<u8>
    ) {
        assert(to.is_non_zero(), 'ERC1155: invalid receiver');
        assert(from.is_non_zero(), 'ERC1155: invalid sender');
        _update(from, to, ids, amounts, data);
    }

    fn _set_uri(uri: felt252) {
        let token = get_contract_address();
        let mut calldata = ArrayTrait::new();
        calldata.append(token.into());
        calldata.append(uri);
        world().execute('ERC1155SetUri'.into(), calldata.span());
    }

    fn _mint(to: ContractAddress, id: u256, amount: u256, data: Array<u8>) {
        assert(to.is_non_zero(), 'ERC1155: invalid receiver');

        let ids = _as_singleton_array(id);
        let amounts = _as_singleton_array(amount);
        _update(Zeroable::zero(), to, ids, amounts, data);
    }

    fn _mint_batch(to: ContractAddress, ids: Array<u256>, amounts: Array<u256>, data: Array<u8>) {
        assert(to.is_non_zero(), 'ERC1155: invalid receiver');
        _update(Zeroable::zero(), to, ids, amounts, data)
    }

    fn _burn(from: ContractAddress, id: u256, amount: u256, data: Array<u8>) {
        assert(from.is_non_zero(), 'ERC1155: invalid sender');

        let ids = _as_singleton_array(id);
        let amounts = _as_singleton_array(amount);
        _update(from, Zeroable::zero(), ids, amounts, data);
    }

    fn _burn_batch(from: ContractAddress, ids: Array<u256>, amounts: Array<u256>, data: Array<u8>) {
        assert(from.is_non_zero(), 'ERC1155: invalid sender');
        _update(from, Zeroable::zero(), ids, amounts, data);
    }

    fn _set_approval_for_all(owner: ContractAddress, operator: ContractAddress, approved: bool) {
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
        world().execute('ERC1155SetApprovalForAll'.into(), calldata.span());

        ApprovalForAll(owner, operator, approved);
    }

    fn _do_safe_transfer_acceptance_check(
        operator: ContractAddress,
        from: ContractAddress,
        to: ContractAddress,
        id: u256,
        amount: u256,
        data: Array<u8>
    ) {
        if (IERC165Dispatcher { contract_address: to }.supports_interface(INTERFACE_ERC1155_RECEIVER)) {
            assert(
                IERC1155TokenReceiverDispatcher { contract_address: to }.on_erc1155_received(
                    operator, from, id, amount, data
                ) == ON_ERC1155_RECEIVED_SELECTOR,
               'ERC1155: ERC1155Receiver reject'
            );
            return ();
        }
        assert(
            IERC165Dispatcher { contract_address: to }.supports_interface( IACCOUNT_ID ),
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
        if (IERC165Dispatcher { contract_address: to }.supports_interface(INTERFACE_ERC1155_RECEIVER)) {
            assert(
                IERC1155TokenReceiverDispatcher { contract_address: to }.on_erc1155_batch_received(
                    operator, from, ids, amounts, data
                ) == ON_ERC1155_BATCH_RECEIVED_SELECTOR,
               'ERC1155: ERC1155Receiver reject'
            );
            return ();
        }
        assert(
            IERC165Dispatcher { contract_address: to }.supports_interface( IACCOUNT_ID ),
            'Transfer to non-ERC1155Receiver'
        );
    }

    fn _as_singleton_array(element: u256) -> Array<u256> {
        let mut array = ArrayTrait::new();
        array.append(element);
        array
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