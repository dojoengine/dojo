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
    use dojo::syscalls::replace_class;
    use starknet::get_contract_address;
    use super::IUpgradableDispatcher;
    use super::IUpgradableDispatcherTrait;

    #[external]
    fn initialize(class_hash: felt252, init_calldata: Array<felt252>) {
        let self_address = get_contract_address();
        replace_class(class_hash);
        IUpgradableDispatcher { contract_address: self_address }.initialize(init_calldata);
    }
}
