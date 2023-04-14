#[system]
mod ERC721Approve {
    use starknet::ContractAddress;
    use super::components::TokenApproval;

    fn execute(address: ContractAddress, token_id: felt252) {
        // approve an address
        commands::set_entity(token_id, (TokenApproval { address }));
    }
}

#[system]
mod ERC721SetApprovalForAll {
    use starknet::ContractAddress;
    use super::components::OperatorApproval;

    fn execute(owner: ContractAddress, operator: ContractAddress, approval: bool) {
        commands::set_entity((owner, operator).into(), (
           OperatorApproval { value: approval } 
        ));
    }
}

#[system]
mod ERC721TransferFrom {
    use starknet::ContractAddress;

    fn execute(from: ContractAddress, to: ContractAddress, token_id: felt252) {
        // reset approvals

        // update balance

        // update ownership



        // reset approvals
        token_approvals::write(token_id, Zeroable::zero());

        // update balances
        let owner_balance = balances::read(from);
        balances::write(from, owner_balance - 1.into());
        let receiver_balance = balances::read(to);
        balances::write(to, receiver_balance + 1.into());

        // update ownership
        owners::write(token_id, to);


    }
}

#[system]
mod ERC721Mint {
    use starknet::contract_address;
    use starknet::ContractAddress;
    use super::components::Owner;
    use super::components::Balance;

    fn execute(owner: ContractAddress, token_id: felt252) {
        // assign token to owner
        commands::set_entity(token_id, ( Owner { address: owner }));

        // update owner's balance
        let balance = commands::<Balance>::entity(owner.into());
        let balance = Balance { value: balance.value + 1 };
        commands::entity(owner.into(), (balance));
    }
}

#[system]
mod ERC721Burn {
    use starknet::contract_address;
    use starknet::ContractAddress;

    fn execute(owner: ContractAddress, token_id: felt252) {
        // remove token from owner
        commands::delete_entity(token_id, ( Owner { address: owner }));

        // update owner's balance
        let balance = commands::<Balance>::entity(owner.into());
        let balance = Balance { value: balance.value - 1 };
        commands::entity(owner.into(), (balance));
    }
}
