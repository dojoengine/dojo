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
mod ERC1155SafeTransferFrom {
    use starknet::get_caller_address;
    use traits::Into;
    use dojo_erc::erc1155::components::Balance;
    use array::ArrayTrait;

    // TODO add data arg
    fn execute(token: felt252, from: felt252, to: felt252, id: felt252, amount: felt252) {
        let from_balance = commands::<Balance>::entity((token, (id, from)).into_partitioned());
        let amount256: u256 = amount.into();   
        assert(from_balance.amount.into() >= amount256, 'ERC1155: insufficient balance');
        commands::set_entity((token, (id, from)).into_partitioned(), (
            Balance { amount: from_balance.amount - amount }
        ));
        let to_balance = commands::<Balance>::entity((token, (id, from)).into_partitioned());   
        commands::set_entity((token, (id, to)).into_partitioned(), (
            Balance { amount: to_balance.amount + amount }
        ));
    }
}

#[system]
mod ERC1155SafeBatchTransferFrom {
    use starknet::get_caller_address;
    use traits::Into;
    use dojo_erc::erc1155::components::Balance;
    use array::ArrayTrait;

    // TODO add data arg
    fn execute(token: felt252, from: felt252, to: felt252, ids: Array<felt252>, amounts: Array<felt252>) {
        let operator = get_caller_address();

        let mut index = 0;
        loop {
            if index == ids.len() {
                break();
            }
            let id = *ids.at(index);
            let amount = *amounts.at(index);

            let from_balance = commands::<Balance>::entity((token, (id, from)).into_partitioned());
            let amount256: u256 = amount.into();   
            assert(from_balance.amount.into() >= amount256, 'ERC1155: insufficient balance');
            commands::set_entity((token, (id, from)).into_partitioned(), (
                Balance { amount: from_balance.amount - amount }
            ));
            let to_balance = commands::<Balance>::entity((token, (id, from)).into_partitioned());   
            commands::set_entity((token, (id, to)).into_partitioned(), (
                Balance { amount: to_balance.amount + amount }
            ));
            index += 1;
        }
    }
}

#[system]
mod ERC1155Mint {
    use traits::Into;
    use dojo_erc::erc1155::components::Balance;

    // TODO add data arg
    fn execute(token: felt252, to: felt252, id: felt252, amount: felt252) {
        let balance = commands::<Balance>::entity((token, (id, to)).into_partitioned());
        commands::set_entity((token, (to)).into_partitioned(), (
            Balance { amount: balance.amount + amount }
        ));
    }
}

#[system]
mod ERC1155MintBatch {
    use traits::Into;
    use dojo_erc::erc1155::components::Balance;
    use array::ArrayTrait;

    // TODO add data arg
    fn execute(token: felt252, to: felt252, ids: Array<felt252>, amounts: Array<felt252>) {
        let mut index = 0;
        loop {
            if index == ids.len() {
                break();
            }
            let id = *ids.at(index);
            let amount = *amounts.at(index);

            let balance = commands::<Balance>::entity((token, (id, to)).into_partitioned());
            commands::set_entity((token, (to)).into_partitioned(), (
                Balance { amount: balance.amount + amount }
            ));
            index += 1;
        }
    }
}

#[system]
mod ERC1155Burn {
    use traits::Into;
    use dojo_erc::erc1155::components::Balance;

    fn execute(token: felt252, from: felt252, id: felt252, amount: felt252) {
        let balance = commands::<Balance>::entity((token, (id, from)).into_partitioned());
        let amount256: u256 = amount.into();
        assert(balance.amount.into() >= amount256, 'ERC1155: burn from 0');
        commands::set_entity((token, (id, from)).into_partitioned(), (
            Balance { amount: balance.amount - amount }
        ));
    }
}

#[system]
mod ERC1155BurnBatch {
    use traits::Into;
    use dojo_erc::erc1155::components::Balance;
    use array::ArrayTrait;

    fn execute(token: felt252, from: felt252, ids: Array<felt252>, amounts: Array<felt252>) {
        let mut index = 0;
        loop {
            if index == ids.len() {
                break();
            }
            let id = *ids.at(index);
            let amount = *amounts.at(index);

            let balance = commands::<Balance>::entity((token, (id, from)).into_partitioned());
            let amount256: u256 = amount.into();
            assert(balance.amount.into() >= amount256, 'ERC1155: burn from 0');
            commands::set_entity((token, (id, from)).into_partitioned(), (
                Balance { amount: balance.amount - amount }
            ));
            index += 1;
        }
    }
}