#[system]
mod ERC721Approve {
    use starknet::ContractAddress;

    fn execute(approved: ContractAddress, token_id: felt252) {
    }
}

#[system]
mod ERC721SetApprovalForAll {
    use starknet::ContractAddress;

    fn execute(operator: ContractAddress, approval: bool) {
    }
}

#[system]
mod ERC721TransferFrom {
    use starknet::ContractAddress;

    fn execute(from: ContractAddress, to: ContractAddress, token_id: felt252) {
    }
}

#[system]
mod ERC721SafeTransferFrom {
    use starknet::ContractAddress;

    fn execute(from: ContractAddress, to: ContractAddress, token_id: felt252, data: Array<felt252>) {
    }
}

#[system]
mod ERC721Mint {
    use starknet::contract_address;
    use starknet::ContractAddress;
    use components::Owner;
    use components::Balance;

    fn execute(owner: ContractAddress, token_id: felt252) {
        // assign token to owner
        commands::set_entity(token_id, ( Owner { address: owner }));

        // update owner's balance
        let balance = commands::<Balance>::entity(owner.into());
        let balance = Balance { value: balance.value + 1 };
        commands::entity(owner.into(), (balance));
    }
}
