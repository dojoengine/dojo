#[system]
mod ERC721Approve {
    use starknet::ContractAddress;
    use traits::{Into, TryInto};
    use option::{OptionTrait};

    use dojo::world::Context;
    use dojo_erc::erc721::components::{
        ERC721OwnerTrait, ERC721TokenApprovalTrait, OperatorApprovalTrait
    };
    use dojo_erc::erc721::erc721::ERC721;
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
    // TODO : emit events
    }
}

#[system]
mod ERC721SetApprovalForAll {
    use starknet::ContractAddress;
    use traits::Into;
    use dojo::world::Context;

    use dojo_erc::erc721::components::{OperatorApprovalTrait};
    use dojo_erc::erc721::erc721::{
        IERC721EventEmitterDispatcher, IERC721EventEmitterDispatcherTrait, ApprovalForAll
    };

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
    // TODO: emit event
    // let event = ApprovalForAll { owner, operator, approved };
    // IDojoERC721Dispatcher { contract_address: token }.on_approval_for_all(event.clone());
    // emit!(ctx.world, event);
    }
}

#[system]
mod ERC721TransferFrom {
    use starknet::ContractAddress;
    use traits::Into;
    use zeroable::Zeroable;

    use dojo::world::Context;
    use dojo_erc::erc721::components::{
        OperatorApprovalTrait, ERC721BalanceTrait, ERC721TokenApprovalTrait, ERC721OwnerTrait,
    };
    use dojo_erc::erc721::erc721::{
        ERC721, IERC721EventEmitterDispatcher, IERC721EventEmitterDispatcherTrait, ApprovalForAll
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
    // TODO: emit event
    // let event = ApprovalForAll { owner, operator, approved };
    // IDojoERC721Dispatcher { contract_address: token }.on_approval_for_all(event.clone());
    // emit!(ctx.world, event);
    }
}

#[system]
mod ERC721Mint {
    use starknet::ContractAddress;
    use traits::Into;
    use zeroable::Zeroable;

    use dojo::world::Context;
    use dojo_erc::erc721::components::{ERC721BalanceTrait, ERC721OwnerTrait};

    fn execute(
        ctx: Context, token: ContractAddress, recipient: ContractAddress, token_id: felt252
    ) {
        assert(token == ctx.origin, 'ERC721: not authorized');
        assert(recipient.is_non_zero(), 'ERC721: mint to 0');

        let owner = ERC721OwnerTrait::owner_of(ctx.world, token, token_id);
        assert(owner.is_zero(), 'ERC721: already minted');

        ERC721BalanceTrait::mint(ctx.world, token, recipient, 1);
        ERC721OwnerTrait::set_owner(ctx.world, token, token_id, recipient);
    // TODO: emit event
    }
}

#[system]
mod ERC721Burn {
    use starknet::ContractAddress;
    use traits::Into;
    use zeroable::Zeroable;

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

        ERC721BalanceTrait::burn(ctx.world, token, owner, 1);
        ERC721OwnerTrait::set_owner(ctx.world, token, token_id, Zeroable::zero());
    // TODO: emit event
    }
}

