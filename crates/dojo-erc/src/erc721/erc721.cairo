// TODO: proper safe_transfer_from
//       token URI - wat do?
//       use low level API for execute
//       check for EIP compliance
//       auth?
//       pass in the token: felt252 as the first param to system::execute, since there can be more than 1 in a game; same as erc20
//       try ((get_contract_address(), token_id).into()) - without using into() on get_contract_address
//       maybe some nice / smart way how to use the get_contract_address automatically to build a query? so I don't have to inject it everywhere

// NOTES:
//   * owner_of is less optimal because it calls `owner` twice (once in `validate_token_id`) but
//     the code is cleaner this way so I kept it
//   * not sure yet where the auth asserts should be - in the 721 interface or in systems?

#[contract]
mod ERC721 {
    use array::ArrayTrait;
    use option::OptionTrait;
    use starknet::contract_address;
    use starknet::ContractAddress;
    use starknet::get_caller_address;
    use starknet::get_contract_address;
    use traits::Into;
    use traits::TryInto;
    use zeroable::Zeroable;

    use dojo_core::interfaces::IWorldDispatcher;
    use dojo_core::interfaces::IWorldDispatcherTrait;

    use super::components::Balance;
    use super::components::OpperatorApproval;
    use super::components::Owner;
    use super::components::TokenApproval;

    // ERC 165 interface codes
    const INTERFACE_ERC165: u32 = 0x01ffc9a7_u32;
    const INTERFACE_ERC721: u32 = 0x80ac58cd_u32;
    const INTERFACE_ERC721_METADATA: u32 = 0x5b5e139f_u32;
    const INTERFACE_ERC721_RECEIVER: u32 = 0x150b7a02_u32;

    #[abi]
    trait IERC721TokenReceiver {
        fn on_erc721_received(operator: addr, from: addr, token_id: u128, data: Array<u8>) -> u32;
    }

    #[abi]
    trait IERC165 {
        fn supports_interface(interface_id: u32) -> bool;
    }

    #[event]
    fn Transfer(from: ContractAddress, to: ContractAddress, token_id: u256) {}

    #[event]
    fn Approval(owner: ContractAddress, approved: ContractAddress, token_id: u256) {}

    #[event]
    fn ApprovalForAll(owner: ContractAddress, operator: ContractAddress, approved: bool) {}

    struct Storage {
        world: ContractAddress,
        name: felt252,
        symbol: felt252,
    }

    //
    // Constructor
    //

    #[constructor]
    fn constructor(world_addr: ContractAddress, token_name: felt252, token_symbol: felt252) {
        world::write(world_addr);
        name::write(token_name);
        symbol::write(token_symbol);
    }

    //
    // ERC721Metadata
    //

    #[view]
    fn name() -> felt252 {
        name::read()
    }

    #[view]
    fn symbol() -> felt252 {
        symbol::read()
    }

    #[view]
    fn token_uri(token_id: u256) -> felt252 {
        let _ = validate_token_id(token_id);
        // TODO
        'https:://dojoengine.org'        
    }

    //
    // ERC721
    //

    #[view]
    fn balance_of(owner: ContractAddress) -> u256 {
        assert_valid_address(owner);
        let balance = commands::<Balance>::entity((get_contract_address().into(), owner));
        balance.value.into()
    }

    #[view]
    fn owner_of(token_id: u256) -> ContractAddress {
        let token_id = validate_token_id(token_id);
        owner(token_id)
    }

    #[view]
    fn get_approved(token_id: u256) -> ContractAddress {
        let token_id = validate_token_id(token_id);
        let approval = commands::<TokenApproval>::entity((get_contract_address().into(), token_id).into());
        approval.address
    }

    #[view]
    fn is_approved_for_all(owner: ContractAddress, operator: ContractAddress) -> bool {
        let approval = commands::<OperatorApproval>::entity((get_contract_address().into(), owner, operator).into());
        approval.value
    }

    #[external]
    fn safe_transfer_from(from: ContractAddress, to: ContractAddress, token_id: u256, data: Array<felt252>) {
        let can_receive_token = IERC165Dispatcher { contract_address: to }.supports_interface(INTERFACE_ERC721_RECEIVER);
        assert(can_receive_token, 'not supported by receiver');

        transfer(from, to, token_id);

        let confirmation = IERC721TokenReceiverDispatcher { contract_address: to }.on_erc721_received(from, to, token_id, data);
        assert(confirmation == INTERFACE_ERC721_RECEIVER, 'incompatible receiver');
    }

    #[external]
    fn transfer_from(from: ContractAddress, to: ContractAddress, token_id: u256) {
        transfer(from, to, token_id);
    }

    #[external]
    fn approve(approved: ContractAddress, token_id: u256) {
        let token_id = validate_token_id(token_id);
        let owner = owner(token_id);
        assert(owner != approved, 'approval to owner');

        let token: felt252 = get_contract_address().into();
        let caller = get_caller_address();
        let operator_approval = commands::<OperatorApproval>::entity((token, owner, caller).into())
        assert(caller == owner | operator_approval.value, 'not approved');

        let mut calldata = ArrayTrait::new();
        calldata.append(token);
        calldata.append(approved.into());
        calldata.append(token_id);
        // commands::execute(ERC721Approve, calldata.span());
        world().execute('ERC721Approve', calldata.span())

        Approval(owner, approved, token_id.into());
    }

    #[external]
    fn set_approval_for_all(operator: ContractAddress, approval: bool) {
        let owner = get_caller_address();
        assert(owner != operator, 'approval to self');

        let mut calldata = ArrayTrait::new();
        calldata.append(get_contract_address().into());
        calldata.append(owner.into());
        calldata.append(operator.into());
        calldata.append(approval.into());
        //commands::execute(ERC721SetApprovalForAll, calldata.span());
        world().execute('ERC721SetApprovalForAll', calldata.span())

        ApprovalForAll(owner, operator, approval);
    }

    //
    // NFT mint / burn
    //

    #[external]
    fn mint(to: ContractAddress) {
        assert(to.is_non_zero(), 'minting to zero address');
        // TODO: assert can mint

        let token = get_contract_address();
        let token_id = commands::uuid();
        let mut calldata = ArrayTrait::new();
        calldata.append(token);
        calldata.append(to);
        calldata.append(token_id);
        // commands::execute(ERC721Mint, calldata.span());
        world().execute('ERC721Mint', calldata.span())

        Transfer(Zeroable::zero(), to, token_id.into());
    }

    #[external]
    fn burn(token_id: u256) {
        let token_id = validate_token_id(token_id);
        let owner = owner(token_id);
        assert(owner == get_caller_address(), 'caller not owner');

        let token = get_contract_address();
        let mut calldata = ArrayTrait::new();
        calldata.append(token);
        calldata.append(owner);
        calldata.append(token_id);
        // commands::execute(ERC721Burn, calldata.span());
        world().execute('ERC721Burn', calldata.span())

        Transfer(owner, Zeroable::zero(), token_id.into());
    }

    //
    // ERC165
    //

    #[view]
    fn supports_interface(interface_id: u32) -> bool {
        interface_id == INTERFACE_ERC165 |
        interface_id == INTERFACE_ERC721 |
        interface_id == INTERFACE_ERC721_METADATA
    }

    //
    // Internal
    //

    // NOTE: temporary, until we have inline commands outside of systems
    fn world() -> IWorldDispatcher {
        IWorldDispatcher { contract_address: world_address::read() }
    }

    fn assert_approved_or_owner(owner: ContractAddress, operator: ContractAddress, token_id: felt252) {
        let approved = commands::<TokenApproval>::entity((get_contract_address().into(), token_id).into());
        assert(
            operator == owner | operator == approved.address | is_approved_for_all(owner, operator),
            'operation not allowed'
        );
    }

    fn assert_valid_address(address: ContractAddress) {
        assert(address.is_non_zero(), 'invalid address');
    }

    fn validate_token_id(token_id: u256) -> felt252 {
        let token_id: felt252 = token_id.try_into().expect('invalid token ID');
        assert_valid_address(owner(token_id));
        token_id
    }

    #[inline(always)]
    fn owner(token: felt252) -> ContractAddress {
        let owner = commands::<Owner>::entity((get_contract_address().into(), token_id).into());
        owner.address
    }

    fn transfer(from: ContractAddress, to: ContractAddress, token_id: u256) {
        let token = get_contract_address();
        let token_id = validate_token_id(token_id);
        let owner = owner(token_id);

        assert(owner.address == from, 'source not owner');
        assert(to.is_non_zero(), 'transferring to zero');
        assert_approved_or_owner(owner, get_caller_address(), token_id);

        let mut calldata = ArrayTrait::new();
        calldata.append(token.into());
        calldata.append(from.into());
        calldata.append(to.into());
        calldata.append(token_id);
        // commands::execute(ERC721TransferFrom, calldata.span());
        world().execute('ERC721TransferFrom', calldata.span());

        Transfer(from, to, token_id.into());
    }
}
