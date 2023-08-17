use starknet::ContractAddress;
use dojo::world::{IWorldDispatcher, IWorldDispatcherTrait};
use zeroable::Zeroable;
use array::{ArrayTrait, SpanTrait};
use option::OptionTrait;

// re-export components from erc_common
use dojo_erc::erc_common::components::{operator_approval, OperatorApproval, OperatorApprovalTrait};


//
// Uri TODO: use BaseURI from erc_common
//

#[derive(Component, Copy, Drop, Serde, SerdeLen)]
struct Uri {
    #[key]
    token: ContractAddress,
    uri: felt252
}

//
// ERC1155Balance
//

#[derive(Component, Copy, Drop, Serde, SerdeLen)]
struct ERC1155Balance {
    #[key]
    token: ContractAddress,
    #[key]
    token_id: felt252,
    #[key]
    account: ContractAddress,

    amount: u128
}

trait ERC1155BalanceTrait
{
    fn balance_of(world: IWorldDispatcher, token: ContractAddress, account: ContractAddress, id: felt252) -> u128;
    fn transfer_tokens(
        world: IWorldDispatcher,
        token: ContractAddress,
        from: ContractAddress,
        to: ContractAddress,
        ids: Span<felt252>,
        amounts: Span<u128>,
    );
}

impl ERC1155BalanceImpl of ERC1155BalanceTrait
{
    fn balance_of(world: IWorldDispatcher, token: ContractAddress, account: ContractAddress, id: felt252) -> u128 {
        // ERC1155: address zero is not a valid owner
        assert(account.is_non_zero(), 'ERC1155: invalid owner address');
        get!(world, (token, id, account), ERC1155Balance).amount
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
}
