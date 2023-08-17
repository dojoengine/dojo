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
    use dojo_erc::erc1155::components::{OperatorApproval, Uri, ERC1155Balance};
    use dojo_erc::erc1155::interface::{
        IERC1155, IERC1155TokenReceiver, IERC1155TokenReceiverDispatcher,
        IERC1155TokenReceiverDispatcherTrait, IERC165, IERC165Dispatcher, IERC165DispatcherTrait
    };

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
            get!(self.world.read(), (token), Uri).uri
        }

        //
        // ERC1155
        //
        fn balance_of(self: @ContractState, account: ContractAddress, id: u256) -> u256 {
            dojo_erc::erc1155::systems::balance_of(self.world.read(), get_contract_address(), account, id)
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
                batch_balances.append(dojo_erc::erc1155::systems::balance_of(self.world.read(), get_contract_address(), *accounts.at(index), *ids.at(index)));
                index += 1;
            }
        }

        fn is_approved_for_all(
            self: @ContractState, account: ContractAddress, operator: ContractAddress
        ) -> bool {
            dojo_erc::erc1155::systems::is_approved_for_all(self.world.read(), get_contract_address(), account, operator)
        }

        fn set_approval_for_all(
            ref self: ContractState, operator: ContractAddress, approved: bool
        ) {
            let owner = get_caller_address();

            assert(owner != operator, 'ERC1155: wrong approval');

            self.world.read().execute('ERC1155SetApprovalForAll',
                to_calldata(get_contract_address())
                    .plus(owner)
                    .plus(operator)
                    .plus(approved)
                    .data
            );

            self.emit(ApprovalForAll { owner, operator, approved });
        }

        fn safe_transfer_from(
            ref self: ContractState,
            from: ContractAddress,
            to: ContractAddress,
            id: u256,
            amount: u256,
            data: Array<u8>
        ) {
            let idf: felt252 = id.try_into().unwrap();
            let amount128: u128 = amount.try_into().unwrap();
            self.world.read().execute('ERC1155SafeTransferFrom',
                to_calldata(get_caller_address())
                    .plus(get_contract_address())
                    .plus(from)
                    .plus(to)
                    .plus(idf)
                    .plus(amount128)
                    .plus(data)
                    .data
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

            self.world.read().execute('ERC1155SafeBatchTransferFrom',
                to_calldata(get_caller_address())
                    .plus(get_contract_address())
                    .plus(from)
                    .plus(to)
                    .plus(idsf)
                    .plus(amounts128)
                    .plus(data)
                    .data
            );
        }
    }

    #[external(v0)]
    fn owner(self: @ContractState) -> ContractAddress {
        self.owner_.read()
    }

    #[external(v0)]
    fn mint(
        ref self: ContractState, to: ContractAddress, id: u256, amount: u256, data: Array<u8>
    ) {
        self._mint(to, id.try_into().unwrap(), amount.try_into().unwrap(), data);
    }

    #[external(v0)]
    fn mint_batch(
        ref self: ContractState,
        to: ContractAddress,
        mut ids: Array<u256>,
        mut amounts: Array<u256>,
        data: Array<u8>
    ) {
        let mut ids_felt: Array<felt252> = ArrayTrait::new();
        let mut amounts_u128: Array<u128> = ArrayTrait::new();
        loop {
            if ids.len() == 0 {
                break ();
            }
            ids_felt.append(ids.pop_front().unwrap().try_into().unwrap());
            amounts_u128.append(amounts.pop_front().unwrap().try_into().unwrap());
        };
        self._mint_batch(to, ids_felt, amounts_u128, data);
    }
    
    //
    // Internal
    //
    
    #[generate_trait]
    impl PrivateFunctions of PrivateFunctionsTrait {
        fn _set_uri(ref self: ContractState, uri: felt252) {
            let mut calldata = ArrayTrait::new();
            calldata.append(uri);
            self.world.read().execute('ERC1155SetUri'.into(), calldata);
        }

        fn _mint(
            ref self: ContractState, to: ContractAddress, id: felt252, amount: u128, data: Array<u8>
        ) {
            assert(to.is_non_zero(), 'ERC1155: invalid receiver');

            self.world.read().execute('ERC1155Mint',
                to_calldata(get_caller_address())
                    .plus(get_contract_address())
                    .plus(to)
                    .plus(array![id])
                    .plus(array![amount])
                    .plus(data)
                    .data
            );
        }

        fn _mint_batch(
            ref self: ContractState,
            to: ContractAddress,
            ids: Array<felt252>,
            amounts: Array<u128>,
            data: Array<u8>
        ) {
            assert(to.is_non_zero(), 'ERC1155: invalid receiver');
            self.world.read().execute('ERC1155Mint',
                to_calldata(get_caller_address())
                    .plus(get_contract_address())
                    .plus(to)
                    .plus(ids)
                    .plus(amounts)
                    .plus(data)
                    .data
            );
        }

        fn _burn(
            ref self: ContractState, from: ContractAddress, id: felt252, amount: u128
        ) {
            assert(from.is_non_zero(), 'ERC1155: invalid sender');

            self.world.read().execute('ERC1155Burn',
                to_calldata(get_caller_address())
                    .plus(get_contract_address())
                    .plus(from)
                    .plus(array![id])
                    .plus(array![amount])
                    .data
            );
        }

        fn _burn_batch(
            ref self: ContractState,
            from: ContractAddress,
            ids: Array<felt252>,
            amounts: Array<u128>
        ) {
            assert(from.is_non_zero(), 'ERC1155: invalid sender');
            self.world.read().execute('ERC1155Burn',
                to_calldata(get_caller_address())
                    .plus(get_contract_address())
                    .plus(from)
                    .plus(ids)
                    .plus(amounts)
                    .data
            );
        }
    }

    #[derive(Drop)]
    struct ToCallData {
        data: Array<felt252>,
    }

    #[generate_trait]
    impl ToCallDataImpl of ToCallDataTrait {
        fn plus<T, impl TSerde: Serde<T>, impl TD: Drop<T>>(mut self: ToCallData, data: T) -> ToCallData {
            data.serialize(ref self.data);
            self
        }
    }

    fn to_calldata<T, impl TSerde: Serde<T>, impl TD: Drop<T>>(data: T) -> ToCallData {
        let mut calldata: Array<felt252> = ArrayTrait::new();
        data.serialize(ref calldata);
        ToCallData { data: calldata }
    }
}
