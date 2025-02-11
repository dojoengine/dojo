#[starknet::contract]
mod Bank {
    use starknet::storage::StoragePointerWriteAccess;
    use starknet::ContractAddress;

    #[storage]
    struct Storage {
        owner: ContractAddress,
    }

    #[constructor]
    fn constructor(ref self: ContractState, owner: ContractAddress) {
        self.owner.write(owner);
    }
}
