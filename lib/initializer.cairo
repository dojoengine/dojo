#[abi]
trait IUpgradable {
    #[external]
    fn initialize(init_calldata: Array::<felt>);
    #[external]
    fn upgrade(class_hash: felt);
}

// ConstantIntializer patterns allows us to decouple
// a contracts address from its implementations class hash
// and constructor arguments. 
#[contract]
mod ConstantIntializer {
    use dojo::syscalls::replace_class;

    #[external]
    fn initialize(class_hash: felt, init_calldata: Array::<felt>) {
        // TODO: Replace with starknet::get_contract_address once it is ready.
        let self_address = starknet::contract_address_const::<17>();
        replace_class(class_hash);
        super::IUpgradableDispatcher::initialize(self_address, init_calldata);
    }
}
