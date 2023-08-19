#[system]
mod erc721_approve {
    use starknet::ContractAddress;
    use traits::Into;

    use dojo::world::Context;
    use dojo_erc::erc721::components::{Owner, TokenApproval, OperatorApproval};
    use dojo_erc::erc721::erc721::ERC721;
    use zeroable::Zeroable;

    fn execute(
        ctx: Context,
        token: ContractAddress,
        caller: ContractAddress,
        token_id: felt252,
        operator: ContractAddress
    ) {
        assert(token == ctx.origin, 'ERC721: not authorized');

        let token_owner = get!(ctx.world, (token, token_id), Owner);
        assert(token_owner.address.is_non_zero(), 'ERC721: invalid token_id');

        let approval = get!(ctx.world, (token, token_owner.address, caller), OperatorApproval);
        // ERC721: approve caller is not token owner or approved for all 
        assert(caller == token_owner.address || approval.approved, 'ERC721: unauthorized caller');
        set!(ctx.world, TokenApproval { token, token_id, address: operator });
    }
}

#[system]
mod erc721_set_approval_for_all {
    use starknet::ContractAddress;
    use traits::Into;

    use dojo::world::Context;
    use dojo_erc::erc721::components::{OperatorApproval, Owner};

    fn execute(
        ctx: Context,
        token: ContractAddress,
        owner: ContractAddress,
        operator: ContractAddress,
        approved: bool
    ) {
        assert(token == ctx.origin, 'ERC721: not authorized');
        assert(owner != operator, 'ERC721: self approval');

        set!(ctx.world, OperatorApproval { token, owner, operator, approved });
    }
}

#[system]
mod erc721_transfer_from {
    use starknet::ContractAddress;
    use traits::Into;
    use zeroable::Zeroable;

    use dojo::world::Context;
    use dojo_erc::erc721::components::{Balance, TokenApproval, OperatorApproval, Owner};
    use dojo_erc::erc721::erc721::ERC721;

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

        let token_owner = get!(ctx.world, (token, token_id), Owner);
        assert(token_owner.address.is_non_zero(), 'ERC721: invalid token_id');

        let token_approval = get!(ctx.world, (token, token_id), TokenApproval);
        let is_approved = get!(ctx.world, (token, token_owner.address, caller), OperatorApproval);
        assert(
            token_owner.address == caller
                || is_approved.approved
                || token_approval.address == caller,
            'ERC721: unauthorized caller'
        );

        set!(ctx.world, TokenApproval { token, token_id, address: Zeroable::zero() });
        set!(ctx.world, Owner { token, token_id, address: to });

        let mut from_balance = get!(ctx.world, (token, from), Balance);
        from_balance.amount -= 1;
        set!(ctx.world, (from_balance));

        let mut to_balance = get!(ctx.world, (token, to), Balance);
        to_balance.amount += 1;
        set!(ctx.world, (to_balance));
    }
}

#[system]
mod erc721_mint {
    use starknet::ContractAddress;
    use traits::Into;
    use zeroable::Zeroable;

    use dojo::world::Context;
    use dojo_erc::erc721::components::{Balance, Owner};

    fn execute(
        ctx: Context, token: ContractAddress, token_id: felt252, recipient: ContractAddress
    ) {
        assert(token == ctx.origin, 'ERC721: not authorized');
        assert(recipient.is_non_zero(), 'ERC721: mint to 0');

        let token_owner = get!(ctx.world, (token, token_id), Owner);
        assert(token_owner.address.is_zero(), 'ERC721: already minted');

        // increase token supply
        let mut balance = get!(ctx.world, (token, recipient), Balance);
        balance.amount += 1;
        set!(ctx.world, (balance));
        set!(ctx.world, Owner { token, token_id, address: recipient });
    }
}

#[system]
mod erc721_burn {
    use starknet::ContractAddress;
    use traits::Into;
    use zeroable::Zeroable;

    use dojo::world::Context;
    use dojo_erc::erc721::components::{Balance, Owner, OperatorApproval, TokenApproval};

    fn execute(ctx: Context, token: ContractAddress, caller: ContractAddress, token_id: felt252) {
        assert(token == ctx.origin, 'ERC721: not authorized');

        let token_owner = get!(ctx.world, (token, token_id), Owner);
        assert(token_owner.address.is_non_zero(), 'ERC721: invalid token_id');

        let token_approval = get!(ctx.world, (token, token_id), TokenApproval);
        let is_approved = get!(ctx.world, (token, token_owner.address, caller), OperatorApproval);

        assert(
            token_owner.address == caller
                || is_approved.approved
                || token_approval.address == caller,
            'ERC721: unauthorized caller'
        );

        let mut balance = get!(ctx.world, (token, token_owner.address), Balance);
        balance.amount -= 1;
        set!(ctx.world, (balance));
        set!(ctx.world, Owner { token, token_id, address: Zeroable::zero() });
    }
}

