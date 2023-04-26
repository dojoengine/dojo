#[system]
mod ERC721Approve {
    use starknet::ContractAddress;
    use super::components::TokenApproval;

    fn execute(token: felt252, approved: felt252, token_id: felt252) {
        // approve an address
        commands::set_entity((token, token_id), (TokenApproval { address: approved.into() }));
    }
}

#[system]
mod ERC721SetApprovalForAll {
    use starknet::ContractAddress;
    use super::components::OperatorApproval;

    fn execute(token: felt252, owner: felt252, operator: felt252, approval: felt252) {
        commands::set_entity((token, owner, operator).into(), (
           OperatorApproval { value: approval.into() } 
        ));
    }
}

#[system]
mod ERC721TransferFrom {
    use starknet::ContractAddress;
    use zeroable::Zeroable;

    fn execute(token: felt252, from: felt252, to: felt252, token_id: felt252) {
        let query: Query = (token, token_id).into();
        commands::set_entity(query, (
            // reset approvals
            TokenApproval { Zeroable::zero() },
            // update ownership
            Owner { address: to.into() }
        ));

        // update old owner balance
        let query: Query = (token, from).into();
        let balance = commands::<Balance>::entity(query);
        commands::set_entity(query, (
            Balance { value: balance.value - 1 }
        ));

        // update new owner balance
        let query: Query = (token, to).into();
        let balance = commands::<Balance>::entity(query);
        commands::set_entity(query, (
            Balance { value: balance.value + 1 }
        ));
    }
}

#[system]
mod ERC721Mint {
    use starknet::contract_address;
    use starknet::ContractAddress;
    use super::components::Owner;

    fn execute(token: felt252, owner: felt252, token_id: felt252) {
        // assign token to owner
        commands::set_entity((token, token_id), ( Owner { address: owner.into() }));

        // update owner's balance
        let query: Query = (token, owner).into();
        let balance = commands::<Balance>::entity(query);
        commands::set_entity(query, (
            Balance { value: balance.value + 1 }
        ))
    }
}

#[system]
mod ERC721Burn {
    use starknet::contract_address;
    use starknet::ContractAddress;

    fn execute(token: felt252, owner: felt252, token_id: felt252) {
        // remove token from owner
        commands::delete_entity((token, token_id), ( Owner { address: owner.into() }));

        // update owner's balance
        let query: Query = (token, owner).into()
        let balance = commands::<Balance>::entity(query);
        commands::set_entity(query, (
            Balance { value: balance.value - 1 }
        ))
    }
}
