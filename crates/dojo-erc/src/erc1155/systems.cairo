#[system]
mod ERC1155SetApprovalForAll {
    use traits::Into;
    use dojo_erc::erc1155::components::OperatorApproval;

    fn execute(token: felt252, owner: felt252, operator: felt252, approved: bool) {
        commands::set_entity((token, (owner, operator)).into_partitioned(), (
            OperatorApproval { value: approved }
        ))
    }
}

// TODO uri storage may not fit in a single felt
#[system]
mod ERC1155SetUri {
    use traits::Into;
    use dojo_erc::erc1155::components::Uri;

    fn execute(token: felt252, uri: felt252) {
        commands::set_entity(token.into(), (
            Uri { uri }
        ))
    }
}

#[system]
mod ERC1155Update {
    use traits::Into;
    use dojo_erc::erc1155::components::Balance;
    use array::ArrayTrait;
    use zeroable::Zeroable;

    fn execute(
        token: felt252,
        operator: felt252,
        from: felt252,
        to: felt252,
        ids: Array<felt252>,
        amounts: Array<felt252>,
        data: Array<felt252>
    ) {
        let mut index = 0;
        loop {
            if index == ids.len() {
                break();
            }
            let id = *ids.at(index);
            let amount = *amounts.at(index);

            if (from.is_non_zero()) {
                let from_balance = commands::<Balance>::entity((token, (id, from)).into_partitioned());
                let amount256: u256 = amount.into(); 
                assert(from_balance.amount.into() >= amount256, 'ERC1155: insufficient balance');
                commands::set_entity(
                    (token, (id, from)).into_partitioned(),
                    (Balance { amount: from_balance.amount - amount })
                );
            }

            if (to.is_non_zero()) {
                let to_balance = commands::<Balance>::entity((token, (id, from)).into_partitioned());   
                commands::set_entity(
                    (token, (id, to)).into_partitioned(),
                    (Balance { amount: to_balance.amount + amount })
                );
            }
            index += 1;
        };
    }
}