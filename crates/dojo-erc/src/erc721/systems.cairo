#[system]
mod erc721_approve {
    use starknet::ContractAddress;
    use traits::Into;

    use dojo::world::Context;
    use dojo_erc::erc721::components::{Owner, TokenApproval, OperatorApproval};
    use dojo_erc::erc721::erc721::ERC721;

    fn execute(ctx: Context, token: ContractAddress, token_id: felt252, operator: ContractAddress) {
        assert(token == ctx.origin, 'ERC721: not authorized');
        let owner = get !(ctx.world, (token, token_id), Owner);
        let approval = get !(ctx.world, (token, owner.address, operator), OperatorApproval);
        assert(owner.address == operator || approval.approved, 'ERC721: unauthorized caller');
        set !(ctx.world, TokenApproval { token, token_id, address: operator });
    }
}

#[system]
mod erc721_set_approval_for_all {
    use starknet::ContractAddress;
    use traits::Into;

    use dojo::world::Context;
    use dojo_erc::erc721::components::OperatorApproval;

    fn execute(
        ctx: Context,
        token: ContractAddress,
        owner: ContractAddress,
        operator: ContractAddress,
        approved: bool
    ) {
        assert(token == ctx.origin, 'ERC721: not authorized');
        assert(owner != operator, 'ERC721: self approval');
        set !(ctx.world, OperatorApproval { token, owner, operator, approved });
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
        from: ContractAddress,
        to: ContractAddress,
        token_id: felt252
    ) {
        assert(token == ctx.origin, 'ERC721: not authorized');

        let owner = get !(ctx.world, (token, token_id), Owner);
        let is_approved = get !(ctx.world, (token, owner.address, from), OperatorApproval);
        assert(owner.address == from || is_approved.approved, 'ERC721: unauthorized caller');
        assert(!to.is_zero(), 'ERC721: invalid receiver');
        assert(from == owner.address, 'ERC721: wrong sender');

        set !(ctx.world, TokenApproval { token, token_id, address: Zeroable::zero() });

        let mut from_balance = get !(ctx.world, (token, from), Balance);
        from_balance.amount -= 1;
        set !(ctx.world, (from_balance));

        let mut to_balance = get !(ctx.world, (token, to), Balance);
        to_balance.amount += 1;
        set !(ctx.world, (to_balance));
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

        // increase token supply
        let mut balance = get !(ctx.world, (token, recipient), Balance);
        balance.amount += 1;
        set !(ctx.world, (balance));
        set !(ctx.world, Owner { token, token_id, address: recipient });
    }
}

#[system]
mod erc721_burn {
    use starknet::ContractAddress;
    use traits::Into;
    use zeroable::Zeroable;

    use dojo::world::Context;
    use dojo_erc::erc721::components::{Balance, Owner};

    fn execute(ctx: Context, token: ContractAddress, token_id: felt252) {
        assert(token == ctx.origin, 'ERC721: not authorized');

        let owner = get !(ctx.world, (token, token_id), Owner);
        let mut balance = get !(ctx.world, (token, owner.address), Balance);
        balance.amount -= 1;
        set !(ctx.world, (balance));
        set !(ctx.world, Owner { token, token_id, address: Zeroable::zero() });
    }
}
