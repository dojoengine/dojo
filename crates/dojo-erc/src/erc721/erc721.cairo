// TODO: proper safe_transfer_from
//       token URI - wat do?
//       use low level API for execute
//       check for EIP compliance

#[contract]
mod ERC721 {
    use array::ArrayTrait;
    use option::OptionTrait;
    use starknet::contract_address;
    use starknet::ContractAddress;
    use starknet::get_caller_address;
    use traits::Into;
    use traits::TryInto;
    use zeroable::Zeroable;

    use dojo_core::interfaces::IWorldDispatcher;
    use dojo_core::interfaces::IWorldDispatcherTrait;

    use super::components::Balance;
    use super::components::OpperatorApproval;
    use super::components::Owner;
    use super::components::TokenApproval;

    #[event]
    fn Transfer(from: ContractAddress, to: ContractAddress, token_id: u256) {}

    #[event]
    fn Approval(owner: ContractAddress, approved: ContractAddress, token_id: u256) {}

    #[event]
    fn ApprovalForAll(owner: ContractAddress, operator: ContractAddress, approved: bool) {}

    struct Storage {
        world: ContractAddress,
        name: felt252,
        symbol: felt252
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
    // External
    //

    #[external]
    fn mint(to: ContractAddress) {
        assert(to.is_non_zero(), 'minting to zero address');
        // TODO: assert can mint

        let token_id = commands::uuid();
        let mut calldata = ArrayTrait::new();
        calldata.append(to);
        calldata.append(token_id);
        // commands::execute(ERC721Mint, calldata.span());
        IWorldDispatcher { contract_address: world::read() }.execute(
            'ERC721Mint', calldata.span()
        )

        Transfer(Zeroable::zero(), to, token_id);
    }

    #[external]
    fn burn(token_id: u256) {
        // TODO: assert can burn

        let token_id = validate_token(token_id);
        let owner = owner(token_id);
        let mut calldata = ArrayTrait::new();
        calldata.append(owner);
        calldata.append(token_id);
        // commands::execute(ERC721Burn, calldata.span());
        IWorldDispatcher { contract_address: world::read() }.execute(
            'ERC721Burn', calldata.span()
        )


        Transfer(owner, Zeroable::zero(), token_id);
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
        let _ = validate_token(token_id);
        // TODO
        'https:://dojoengine.org'        
    }

    //
    // ERC721
    //

    #[view]
    fn balance_of(owner: ContractAddress) -> u256 {
        assert_valid_address(owner);
        let balance = commands::<Balance>::entity(owner);
        balance.value.into()
    }

    #[view]
    fn owner_of(token_id: u256) -> ContractAddress {
        let token_id = validate_token(token_id);
        owner(token_id)
    }

    #[view]
    fn get_approved(token_id: u256) -> ContractAddress {
        let token_id = validate_token(token_id);
        let approval = commands::<TokenApproval>::entity(token_id);
        approval.address
    }

    #[view]
    fn is_approved_for_all(owner: ContractAddress, operator: ContractAddress) -> bool {
        let approval = commands::<OperatorApproval>::entity((owner, operator).into());
        approval.value
    }

    // #[external]
    // fn safe_transfer_from(from: ContractAddress, to: ContractAddress, token_id: u256, data: Array<felt252>) {
    //     transfer(from, to, token_id);
    //     // TODO: on_erc721_received?
    // }

    #[external]
    fn transfer_from(from: ContractAddress, to: ContractAddress, token_id: u256) {
        transfer(from, to, token_id);
    }

    #[external]
    fn approve(approved: ContractAddress, token_id: u256) {
        let token_id = validate_token(token_id);
        let owner = owner(token_id);
        let caller = get_caller_address();
        let operator_approval = commands::<OperatorApproval>::entity((owner, caller).into())
        assert(caller == owner | operator_approval.value, 'not approved');

        let mut calldata = ArrayTrait::new();
        calldata.append(approved.into());
        calldata.append(token_id);
        // commands::execute(ERC721Approve, calldata.span());
        IWorldDispatcher { contract_address: world::read() }.execute(
            'ERC721Approve', calldata.span()
        )

        Approval(owner, approved, token_id);
    }

    #[external]
    fn set_approval_for_all(operator: ContractAddress, approval: bool) {
        let owner = get_caller_address();
        assert(owner != operator, 'approval to self');

        let mut calldata = ArrayTrait::new();
        calldata.append(owner.into());
        calldata.append(operator.into());
        calldata.append(approval.into());
        //commands::execute(ERC721SetApprovalForAll, calldata.span());
        IWorldDispatcher { contract_address: world::read() }.execute(
            'ERC721SetApprovalForAll', calldata.span()
        )

        ApprovalForAll(owner, operator, approval);
    }

    //
    // ERC165
    //

    #[view]
    fn supports_interface(interface_id: u32) -> bool {
        // ERC165
        interface_id == 0x01ffc9a7_u32 |
        // ERC721
        interface_id == 0x80ac58cd_u32 |
        // ERC721 Metadata
        interface_id == 0x5b5e139f_u32
    }

    //
    // Internal
    //

    fn assert_approved_or_owner(owner: ContractAddress, operator: ContractAddress, token_id: felt252) {
        let approved = commands::<TokenApproval>::entity(token_id);
        assert(
            operator == owner | operator == approved.address | is_approved_for_all(owner, operator),
            'operation not allowed'
        );
    }

    fn assert_valid_address(address: ContractAddress) {
        assert(address.is_non_zero(), 'invalid address');
    }

    fn validate_token(token_id: u256) -> felt252 {
        let token_id: felt252 = token_id.try_into().expect('invalid token ID');
        assert_valid_address(owner(token_id));
        token_id
    }

    #[inline(always)]
    fn owner(token: felt252) -> ContractAddress {
        let owner = commands::<Owner>::entity(token_id);
        owner.address
    }

    fn transfer(from: ContractAddress, to: ContractAddress, token_id: u256) {
        let token_id = validate_token(token_id);
        let owner = commands::<Owner>::entity(token_id);

        assert(owner.address == from, 'source not owner');
        assert(to.is_non_zero(), 'transferring to zero');
        assert_approved_or_owner(owner, get_caller_address(), token_id);

        let mut calldata = ArrayTrait::new();
        calldata.append(from);
        calldata.append(to);
        calldata.append(token_id);
        // commands::execute(ERC721TransferFrom, calldata.span());
        IWorldDispatcher { contract_address: world::read() }.execute(
            'ERC721TransferFrom', calldata.span()
        )

        Transfer(from, to, token_id);
    }
}
