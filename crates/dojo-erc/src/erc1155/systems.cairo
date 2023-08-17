use core::array::SpanTrait;
use starknet::{ContractAddress, get_contract_address};
use zeroable::Zeroable;
use array::ArrayTrait;
use option::OptionTrait;
use serde::Serde;
use traits::{Into, TryInto};

use dojo::world::{IWorldDispatcher, IWorldDispatcherTrait};

use dojo_erc::erc1155::erc1155::ERC1155::{TransferSingle, TransferBatch};
use dojo_erc::erc1155::components::{ERC1155Balance, OperatorApproval};


fn balance_of(world: IWorldDispatcher, token: ContractAddress, account: ContractAddress, id: u256) -> u256 {
    // ERC1155: address zero is not a valid owner
    assert(account.is_non_zero(), 'ERC1155: invalid owner address');

    let id_felt: felt252 = id.try_into().unwrap();
    let balance = get!(world, (token, id_felt, account), ERC1155Balance);
    balance.amount.into()
}

fn is_approved_for_all(
    world: IWorldDispatcher, token: ContractAddress, account: ContractAddress, operator: ContractAddress
) -> bool {
    let approval = get!(world, (token, account, operator), OperatorApproval);
    approval.approved
}

fn transfer_tokens(
    world: IWorldDispatcher,
    token: ContractAddress,
    from: ContractAddress,
    to: ContractAddress,
    mut ids: Span<felt252>,
    mut amounts: Span<u128>,
)
{
    loop {
        if ids.len() == 0 {
            break ();
        }
        let id = *ids.pop_front().unwrap();
        let amount: u128 = *amounts.pop_front().unwrap();

        if (from.is_non_zero()) {
            let mut from_balance = get!(world, (token, id, from), ERC1155Balance);
            from_balance.amount -= amount;
            set!(world, (from_balance));
        }

        if (to.is_non_zero()) {
            let mut to_balance = get!(world, (token, id, to), ERC1155Balance);
            to_balance.amount += amount;
            set!(world, (to_balance));
        };
    };
}

fn update(
    world: IWorldDispatcher,
    operator: ContractAddress,
    token: ContractAddress,
    from: ContractAddress,
    to: ContractAddress,
    ids: Array<felt252>,
    amounts: Array<u128>,
    data: Array<u8>
) {
    assert(ids.len() == amounts.len(), 'ERC1155: invalid length');

    assert(
        operator == from || is_approved_for_all(world, token, from, operator),
        'ERC1155: insufficient approval'
    );

    transfer_tokens(world, token, from, to, ids.span(), amounts.span());
    
    // if (ids.len() == 1) {
    //     let id = *ids.at(0);
    //     let amount = *amounts.at(0);

    //     emit!(world, TransferSingle { operator, from, to, id: id.into(), value: amount.into() });

    //     if (to.is_non_zero()) {
    //         do_safe_transfer_acceptance_check(operator, from, to, id.into(), amount.into(), data);
    //     } else {
    //         emit!(world, TransferBatch {
    //             operator: operator,
    //             from: from,
    //             to: to,
    //             ids: ids.clone(),
    //             values: amounts.clone()
    //         });
    //         if (to.is_non_zero()) {
    //             do_safe_batch_transfer_acceptance_check(
    //                 operator, from, to, ids, amounts, data
    //             );
    //         }
    //     }
    // }
}

fn do_safe_transfer_acceptance_check(
    operator: ContractAddress,
    from: ContractAddress,
    to: ContractAddress,
    id: u256,
    amount: u256,
    data: Array<u8>
) { // if (IERC165Dispatcher {
//     contract_address: to
// }.supports_interface(INTERFACE_ERC1155_RECEIVER)) {
//     assert(
//         IERC1155TokenReceiverDispatcher {
//             contract_address: to
//         }
//             .on_erc1155_received(
//                 operator, from, id, amount, data
//             ) == ON_ERC1155_RECEIVED_SELECTOR,
//         'ERC1155: ERC1155Receiver reject'
//     );
//     return ();
// }
// assert(
//     IERC165Dispatcher { contract_address: to }.supports_interface(IACCOUNT_ID),
//     'Transfer to non-ERC1155Receiver'
// );
}

fn do_safe_batch_transfer_acceptance_check(
    operator: ContractAddress,
    from: ContractAddress,
    to: ContractAddress,
    ids: Array<u256>,
    amounts: Array<u256>,
    data: Array<u8>
) { // if (IERC165Dispatcher {
//     contract_address: to
// }.supports_interface(INTERFACE_ERC1155_RECEIVER)) {
//     assert(
//         IERC1155TokenReceiverDispatcher {
//             contract_address: to
//         }
//             .on_erc1155_batch_received(
//                 operator, from, ids, amounts, data
//             ) == ON_ERC1155_BATCH_RECEIVED_SELECTOR,
//         'ERC1155: ERC1155Receiver reject'
//     );
//     return ();
// }
// assert(
//     IERC165Dispatcher { contract_address: to }.supports_interface(IACCOUNT_ID),
//     'Transfer to non-ERC1155Receiver'
// );
}

#[system]
mod ERC1155SetApprovalForAll {
    use traits::Into;
    use dojo::world::Context;
    use dojo_erc::erc1155::components::OperatorApproval;
    use starknet::ContractAddress;

    fn execute(
        ctx: Context, token: ContractAddress, owner: ContractAddress, operator: ContractAddress, approved: bool
    ) {
        assert(token == ctx.origin, 'ERC1155: not authorized');

        let mut operator_approval = get!(ctx.world, (token, owner, operator), OperatorApproval);
        operator_approval.approved = approved;
        set!(ctx.world, (operator_approval))
    }
}

// TODO uri storage may not fit in a single felt
#[system]
mod ERC1155SetUri {
    use traits::Into;
    use dojo::world::Context;
    use dojo_erc::erc1155::components::Uri;
    use starknet::ContractAddress;

    fn execute(ctx: Context, uri: felt252) {
        // TODO
        //assert(token == ctx.origin, 'ERC1155: not authorized');

        let mut _uri = get!(ctx.world, (ctx.origin), Uri);
        _uri.uri = uri;
        set!(ctx.world, (_uri))
    }
}

#[system]
mod ERC1155SafeTransferFrom {
    use traits::{Into, TryInto};
    use option::OptionTrait;
    use array::ArrayTrait;
    use dojo::world::Context;
    use zeroable::Zeroable;
    use starknet::ContractAddress;

    fn execute(
        ctx: Context,
        operator: ContractAddress,
        token: ContractAddress,
        from: ContractAddress,
        to: ContractAddress,
        id: felt252,
        amount: u128,
        data: Array<u8>
    ) {
        assert(token == ctx.origin, 'ERC1155: not authorized');

        assert(to.is_non_zero(), 'ERC1155: to cannot be 0');

        super::update(ctx.world, operator, token, from, to, array![id], array![amount], data);
    }
}

#[system]
mod ERC1155SafeBatchTransferFrom {
    use traits::{Into, TryInto};
    use option::OptionTrait;
    use array::ArrayTrait;
    use dojo::world::Context;
    use zeroable::Zeroable;
    use starknet::ContractAddress;

    fn execute(
        ctx: Context,
        operator: ContractAddress,
        token: ContractAddress,
        from: ContractAddress,
        to: ContractAddress,
        ids: Array<felt252>,
        amounts: Array<u128>,
        data: Array<u8>
    ) {
        assert(token == ctx.origin, 'ERC1155: not authorized');

        assert(to.is_non_zero(), 'ERC1155: to cannot be 0');

        super::update(ctx.world, operator, token, from, to, ids, amounts, data);
    }
}


#[system]
mod ERC1155Mint {
    use traits::{Into, TryInto};
    use option::OptionTrait;
    use array::ArrayTrait;
    use dojo::world::Context;
    use zeroable::Zeroable;
    use starknet::ContractAddress;

    fn execute(
        ctx: Context,
        operator: ContractAddress,
        token: ContractAddress,
        to: ContractAddress,
        ids: Array<felt252>,
        amounts: Array<u128>,
        data: Array<u8>
    ) {
        assert(token == ctx.origin, 'ERC1155: not authorized');

        super::update(ctx.world, operator, token, Zeroable::zero(), to, ids, amounts, data);
    }
}


#[system]
mod ERC1155Burn {
    use traits::{Into, TryInto};
    use option::OptionTrait;
    use array::ArrayTrait;
    use dojo::world::Context;
    use zeroable::Zeroable;
    use starknet::ContractAddress;

    fn execute(
        ctx: Context,
        operator: ContractAddress,
        token: ContractAddress,
        from: ContractAddress,
        ids: Array<felt252>,
        amounts: Array<u128>
    ) {
        assert(token == ctx.origin, 'ERC1155: not authorized');

        super::update(ctx.world, operator, token, from, Zeroable::zero(), ids, amounts, array![]);
    }
}
