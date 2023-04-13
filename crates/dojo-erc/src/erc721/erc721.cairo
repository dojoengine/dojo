// TODO: proper safe_transfer_from
//       token URI - wat do?

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
        // token_uris: LegacyMap<u256, felt252>,
        // balances: LegacyMap<ContractAddress, u256>,
        // owners: LegacyMap<u256, ContractAddress>,
        // token_approvals: LegacyMap<u256, ContractAddress>,
        // operator_approvals: LegacyMap<(ContractAddress, ContractAddress), bool>,
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

        let token_id = commands::uuid();
        let mut calldata = ArrayTrait::new();
        calldata.append(to);
        calldata.append(token_id);

        // TODO: will this work? if so, not the greatest...
        let world_address = world::read();
        commands::execute(ERC721Mint, calldata.span());

        Transfer(Zeroable::zero(), to, token_id);
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
    fn token_uri(token_id: felt252) -> felt252 {
        assert_valid_token(token_id);
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
        // TODO: improve - the assert does the same entity lookup as 2 line below
        assert_valid_token(token_id);
        let token_id: felt252 = token_id.try_into().unwrap();
        let owner = commands::<Owner>::entity(token_id);
        owner.address
    }

    // #[view]
    // fn get_approved(token_id: u256) -> ContractAddress {
    //     assert_valid_token(token_id);
    //     token_approvals::read(token_id)
    // }

    // #[view]
    // fn is_approved_for_all(owner: ContractAddress, operator: ContractAddress) -> bool {
    //     operator_approvals::read((owner, operator))
    // }

    // #[external]
    // fn safe_transfer_from(from: ContractAddress, to: ContractAddress, token_id: u256, data: Array<felt252>) {
    //     transfer(from, to, token_id);
    //     // TODO: on_erc721_received?
    // }

    // #[external]
    // fn transfer_from(from: ContractAddress, to: ContractAddress, token_id: u256) {
    //     transfer(from, to, token_id);
    // }

    // #[external]
    // fn approve(approved: ContractAddress, token_id: u256) {
    //     let owner = owners::read(token_id);
    //     assert(owner != approved, 'approval to owner');

    //     let caller = get_caller_address();
    //     assert(
    //         caller == owner | operator_approvals::read((owner, caller)), 
    //         'not approved'
    //     );

    //     token_approvals::write(token_id, approved);
    //     Approval(owner, approved, token_id);
    // }

    // #[external]
    // fn set_approval_for_all(operator: ContractAddress, approval: bool) {
    //     let owner = get_caller_address();
    //     assert(owner != operator, 'approval to self');
    //     operator_approvals::write((owner, operator), approval);
    //     ApprovalForAll(owner, operator, approval);
    // }

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

    // fn assert_approved_or_owner(operator: addr, token_id: u256) {
    //     let owner = owners::read(token_id);
    //     let approved = get_approved(token_id);
    //     assert(
    //         operator == owner | operator == approved | is_approved_for_all(owner, operator),
    //         'operation not allowed'
    //     );
    // }

    fn assert_valid_address(address: ContractAddress) {
        assert(address.is_non_zero(), 'invalid address');
    }

    fn assert_valid_token(token_id: u256) {
        let token_id: felt252 = token_id.try_into().expect('invalid token ID');
        let owner = commands::<Owner>::entity(token_id);
        assert_valid_address(owner.address);
    }

    fn transfer(from: addr, to: addr, token_id: u256) {
        assert_approved_or_owner(get_caller_address(), token_id);
        assert(owners::read(token_id) == from, 'source not owner');
        assert(to.is_non_zero(), 'transferring to zero');
        assert_valid_token(token_id);

        // reset approvals
        token_approvals::write(token_id, Zeroable::zero());

        // update balances
        let owner_balance = balances::read(from);
        balances::write(from, owner_balance - 1.into());
        let receiver_balance = balances::read(to);
        balances::write(to, receiver_balance + 1.into());

        // update ownership
        owners::write(token_id, to);
        Transfer(from, to, token_id);
    }
}
