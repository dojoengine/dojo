use core::array::SpanTrait;
use starknet::{ContractAddress, get_contract_address};
use zeroable::Zeroable;
use array::ArrayTrait;
use option::OptionTrait;
use serde::Serde;
use clone::Clone;
use traits::{Into, TryInto};

use dojo::world::{IWorldDispatcher, IWorldDispatcherTrait};

use dojo_erc::erc1155::erc1155::{TransferSingle, TransferBatch};
use dojo_erc::erc1155::erc1155::{IDojoERC1155Dispatcher, IDojoERC1155DispatcherTrait};
use dojo_erc::erc1155::components::{ERC1155BalanceTrait, OperatorApprovalTrait};

fn emit_transfer_single(
    world: IWorldDispatcher,
    token: ContractAddress,
    operator: ContractAddress,
    from: ContractAddress,
    to: ContractAddress,
    id: felt252,
    amount: u128
) {
    let event = TransferSingle { operator, from, to, id: id.into(), value: amount.into() };
    IDojoERC1155Dispatcher { contract_address: token }.on_transfer_single(event.clone());
    emit!(world, event);
}

fn emit_transfer_batch(
    world: IWorldDispatcher,
    token: ContractAddress,
    operator: ContractAddress,
    from: ContractAddress,
    to: ContractAddress,
    mut ids: Span<felt252>,
    mut amounts: Span<u128>
) {
    let mut ids_u256: Array<u256> = ArrayTrait::new();
    let mut amounts_u256: Array<u256> = ArrayTrait::new();
    loop {
        if ids.len() == 0 {
            break;
        }
        ids_u256.append((*ids.pop_front().unwrap()).into());
        amounts_u256.append((*amounts.pop_front().unwrap()).into());
    };
    let event = TransferBatch {
        operator: operator, from: from, to: to, ids: ids_u256, values: amounts_u256, 
    };
    IDojoERC1155Dispatcher { contract_address: token }.on_transfer_batch(event.clone());
    emit!(world, event);
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
        operator == from
            || OperatorApprovalTrait::is_approved_for_all(world, token, from, operator),
        'ERC1155: insufficient approval'
    );

    ERC1155BalanceTrait::transfer_tokens(world, token, from, to, ids.span(), amounts.span());

    if (ids.len() == 1) {
        let id = *ids.at(0);
        let amount = *amounts.at(0);

        emit_transfer_single(world, token, operator, from, to, id, amount);

        if (to.is_non_zero()) {//do_safe_transfer_acceptance_check(operator, from, to, id.into(), amount.into(), data);
        } else {
            emit_transfer_batch(world, token, operator, from, to, ids.span(), amounts.span());
            if (to.is_non_zero()) {//do_safe_batch_transfer_acceptance_check(
            //    operator, from, to, ids, amounts, data
            //);
            }
        }
    }
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
    use starknet::ContractAddress;
    use array::ArrayTrait;
    use clone::Clone;

    use dojo_erc::erc1155::components::OperatorApprovalTrait;
    use dojo_erc::erc1155::erc1155::{
        IDojoERC1155Dispatcher, IDojoERC1155DispatcherTrait, ApprovalForAll
    };
    fn execute(
        ctx: Context,
        token: ContractAddress,
        owner: ContractAddress,
        operator: ContractAddress,
        approved: bool
    ) {
        // TODO : safety checks !!

        OperatorApprovalTrait::set_approval_for_all(ctx.world, token, owner, operator, approved);

        let event = ApprovalForAll { owner, operator, approved };
        IDojoERC1155Dispatcher { contract_address: token }.on_approval_for_all(event.clone());
        emit!(ctx.world, event);
    }
}

// TODO uri storage may not fit in a single felt
#[system]
mod ERC1155SetUri {
    use traits::Into;
    use dojo::world::Context;
    use dojo_erc::erc1155::components::Uri;
    use starknet::ContractAddress;

    fn execute(ctx: Context, token: ContractAddress, uri: felt252) {
        assert(ctx.origin == token, 'ERC1155: not authorized');
        let mut _uri = get!(ctx.world, (token), Uri);
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
        token: ContractAddress,
        operator: ContractAddress,
        from: ContractAddress,
        to: ContractAddress,
        id: felt252,
        amount: u128,
        data: Array<u8>
    ) {
        assert(ctx.origin == operator || ctx.origin == token, 'ERC1155: not authorized');
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
        assert(ctx.origin == operator || ctx.origin == token, 'ERC1155: not authorized');
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
        token: ContractAddress,
        operator: ContractAddress,
        to: ContractAddress,
        ids: Array<felt252>,
        amounts: Array<u128>,
        data: Array<u8>
    ) {
        assert(ctx.origin == operator || ctx.origin == token, 'ERC1155: not authorized');
        assert(to.is_non_zero(), 'ERC1155: invalid receiver');

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
        token: ContractAddress,
        operator: ContractAddress,
        from: ContractAddress,
        ids: Array<felt252>,
        amounts: Array<u128>
    ) {
        assert(ctx.origin == operator || ctx.origin == token, 'ERC1155: not authorized');
        assert(from.is_non_zero(), 'ERC1155: invalid sender');

        super::update(ctx.world, operator, token, from, Zeroable::zero(), ids, amounts, array![]);
    }
}
