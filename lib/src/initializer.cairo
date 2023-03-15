#[abi]
trait IUpgradable {
    #[external]
    fn initialize(init_calldata: Array<felt252>);
    #[external]
    fn upgrade(class_hash: felt252);
}

// ConstantIntializer patterns allows us to decouple
// a contracts address from its implementations class hash
// and constructor arguments. 
#[contract]
mod ConstantIntializer {
    use starknet::syscalls::replace_class_syscall;
    use starknet::get_contract_address;
    use super::IUpgradableDispatcher;
    use super::IUpgradableDispatcherTrait;

    #[external]
    fn initialize(class_hash: starknet::ClassHash, init_calldata: Array<felt252>) {
        let self_address = get_contract_address();
        replace_class_syscall(class_hash).unwrap_syscall();
        IUpgradableDispatcher { contract_address: self_address }.initialize(init_calldata);
    }
}
