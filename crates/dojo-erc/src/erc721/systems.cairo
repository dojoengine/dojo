use array::{ArrayTrait, SpanTrait};
use option::OptionTrait;
use serde::Serde;
use clone::Clone;
use traits::{Into, TryInto};
use starknet::{ContractAddress, get_contract_address};
use dojo::world::{IWorldDispatcher, IWorldDispatcherTrait};
use dojo_erc::erc721::erc721::ERC721;

use dojo_erc::erc721::erc721::ERC721::{
    IERC721EventEmitterDispatcher, IERC721EventEmitterDispatcherTrait, Approval, Transfer,
    ApprovalForAll
};

fn emit_transfer(
    world: IWorldDispatcher,
    token: ContractAddress,
    from: ContractAddress,
    to: ContractAddress,
    token_id: felt252,
) {
    let event = Transfer { from, to, token_id: token_id.into() };
    IERC721EventEmitterDispatcher { contract_address: token }.on_transfer(event.clone());
    emit!(world, event);
}

fn emit_approval(
    world: IWorldDispatcher,
    token: ContractAddress,
    owner: ContractAddress,
    to: ContractAddress,
    token_id: felt252,
) {
    let event = Approval { owner, to, token_id: token_id.into() };
    IERC721EventEmitterDispatcher { contract_address: token }.on_approval(event.clone());
    emit!(world, event);
}


fn emit_approval_for_all(
    world: IWorldDispatcher,
    token: ContractAddress,
    owner: ContractAddress,
    operator: ContractAddress,
    approved: bool,
) {
    let event = ApprovalForAll { owner, operator, approved };
    IERC721EventEmitterDispatcher { contract_address: token }.on_approval_for_all(event.clone());
    emit!(world, event);
}


#[system]
mod ERC721Approve {
    use starknet::ContractAddress;
    use traits::{Into, TryInto};
    use option::{OptionTrait};
    use clone::Clone;
    use array::{ArrayTrait, SpanTrait};

    use dojo::world::Context;
    use dojo_erc::erc721::components::{
        ERC721OwnerTrait, ERC721TokenApprovalTrait, OperatorApprovalTrait
    };
    use super::{IERC721EventEmitterDispatcher, IERC721EventEmitterDispatcherTrait, Approval};
    use zeroable::Zeroable;

    fn execute(
        ctx: Context,
        token: ContractAddress,
        caller: ContractAddress,
        token_id: felt252,
        to: ContractAddress
    ) {
        assert(token == ctx.origin, 'ERC721: not authorized');

        let owner = ERC721OwnerTrait::owner_of(ctx.world, token, token_id);
        assert(owner.is_non_zero(), 'ERC721: invalid token_id');

        let is_approved_for_all = OperatorApprovalTrait::is_approved_for_all(
            ctx.world, token, owner, caller
        );
        // // ERC721: approve caller is not token owner or approved for all 
        assert(caller == owner || is_approved_for_all, 'ERC721: unauthorized caller');
        ERC721TokenApprovalTrait::approve(ctx.world, token, token_id, to, );

        // emit events
        super::emit_approval(ctx.world, token, owner, to, token_id);
    }
}

#[system]
mod ERC721SetApprovalForAll {
    use starknet::ContractAddress;
    use traits::Into;
    use dojo::world::Context;
    use clone::Clone;
    use array::{ArrayTrait, SpanTrait};

    use dojo_erc::erc721::components::{OperatorApprovalTrait};

    fn execute(
        ctx: Context,
        token: ContractAddress,
        owner: ContractAddress,
        operator: ContractAddress,
        approved: bool
    ) {
        // assert(token == ctx.origin, 'ERC721: not authorized');
        // assert(owner != operator, 'ERC721: self approval');

        // TODO : safety checks !!
        OperatorApprovalTrait::set_approval_for_all(ctx.world, token, owner, operator, approved);

        // emit event
        super::emit_approval_for_all(ctx.world, token, owner, operator, approved);
    }
}

#[system]
mod ERC721TransferFrom {
    use starknet::ContractAddress;
    use traits::Into;
    use zeroable::Zeroable;
    use array::SpanTrait;

    use dojo::world::Context;
    use dojo_erc::erc721::components::{
        OperatorApprovalTrait, ERC721BalanceTrait, ERC721TokenApprovalTrait, ERC721OwnerTrait,
    };

    fn execute(
        ctx: Context,
        token: ContractAddress,
        caller: ContractAddress,
        from: ContractAddress,
        to: ContractAddress,
        token_id: felt252
    ) {
        assert(token == ctx.origin, 'ERC721: not authorized');
        assert(!to.is_zero(), 'ERC721: invalid receiver');

        let owner = ERC721OwnerTrait::owner_of(ctx.world, token, token_id);
        assert(owner.is_non_zero(), 'ERC721: invalid token_id');

        let is_approved_for_all = OperatorApprovalTrait::is_approved_for_all(
            ctx.world, token, owner, caller
        );
        let approved = ERC721TokenApprovalTrait::get_approved(ctx.world, token, token_id);

        assert(
            owner == caller || is_approved_for_all || approved == caller,
            'ERC721: unauthorized caller'
        );

        ERC721OwnerTrait::set_owner(ctx.world, token, token_id, to);
        ERC721BalanceTrait::transfer_token(ctx.world, token, from, to, 1);
        ERC721TokenApprovalTrait::approve(ctx.world, token, token_id, Zeroable::zero());

        // emit events
        super::emit_transfer(ctx.world, token, from, to, token_id);
    }
}

#[system]
mod ERC721Mint {
    use starknet::ContractAddress;
    use traits::Into;
    use zeroable::Zeroable;
    use array::SpanTrait;

    use dojo::world::Context;
    use dojo_erc::erc721::components::{ERC721BalanceTrait, ERC721OwnerTrait};


    fn execute(
        ctx: Context, token: ContractAddress, recipient: ContractAddress, token_id: felt252
    ) {
        assert(token == ctx.origin, 'ERC721: not authorized');
        assert(recipient.is_non_zero(), 'ERC721: mint to 0');

        let owner = ERC721OwnerTrait::owner_of(ctx.world, token, token_id);
        assert(owner.is_zero(), 'ERC721: already minted');

        ERC721BalanceTrait::increase_balance(ctx.world, token, recipient, 1);
        ERC721OwnerTrait::set_owner(ctx.world, token, token_id, recipient);
        // emit events
        super::emit_transfer(ctx.world, token, Zeroable::zero(), recipient, token_id);
    }
}

#[system]
mod ERC721Burn {
    use starknet::ContractAddress;
    use traits::Into;
    use zeroable::Zeroable;
    use array::SpanTrait;

    use dojo::world::Context;
    use dojo_erc::erc721::components::{
        ERC721BalanceTrait, ERC721OwnerTrait, ERC721TokenApprovalTrait, OperatorApprovalTrait,
    };

    fn execute(ctx: Context, token: ContractAddress, caller: ContractAddress, token_id: felt252) {
        assert(token == ctx.origin, 'ERC721: not authorized');

        let owner = ERC721OwnerTrait::owner_of(ctx.world, token, token_id);
        assert(!owner.is_zero(), 'ERC721: invalid token_id');

        let is_approved_for_all = OperatorApprovalTrait::is_approved_for_all(
            ctx.world, token, owner, caller
        );
        let approved = ERC721TokenApprovalTrait::get_approved(ctx.world, token, token_id);

        assert(
            owner == caller || is_approved_for_all || approved == caller,
            'ERC721: unauthorized caller'
        );

        ERC721BalanceTrait::decrease_balance(ctx.world, token, owner, 1);
        ERC721OwnerTrait::set_owner(ctx.world, token, token_id, Zeroable::zero());

        //  emit events
        super::emit_transfer(ctx.world, token, owner, Zeroable::zero(), token_id);
    }
}


// TODO: move in erc_common

#[system]
mod ERC721SetBaseUri {
    use traits::Into;
    use dojo::world::Context;
    use dojo_erc::erc_common::components::{BaseUri, BaseUriTrait};
    use starknet::ContractAddress;

    fn execute(ctx: Context, token: ContractAddress, uri: felt252) {
        assert(ctx.origin == token, 'ERC721: not authorized');
        BaseUriTrait::set_base_uri(ctx.world, token, uri);
    // TODO: emit event
    }
}
