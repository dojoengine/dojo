use dojo::utils::operator::OperatorMode;
use starknet::SyscallResultTrait;
use dojo::utils::{IOperatorDispatcher, IOperatorDispatcherTrait};
use snforge_std::{ContractClassTrait, DeclareResultTrait};
use starknet::{ClassHash, ContractAddress};

#[starknet::interface]
pub trait IMockContract<T> {
    fn test_call(ref self: T);
}

#[starknet::contract]
mod mock_contract {
    use dojo::utils::OperatorComponent as operator_cpt;
    use dojo::utils::OperatorComponent::InternalTrait as OperatorInternal;

    #[storage]
    struct Storage {
        #[substorage(v0)]
        operator: operator_cpt::Storage,
    }

    #[event]
    #[derive(Drop, starknet::Event)]
    enum Event {
        #[flat]
        OperatorEvent: operator_cpt::Event,
    }

    component!(path: operator_cpt, storage: operator, event: OperatorEvent);

    #[constructor]
    fn constructor(ref self: ContractState) {
        self.operator.initialize(starknet::get_caller_address());
    }

    #[abi(embed_v0)]
    impl OperatorImpl = operator_cpt::OperatorImpl<ContractState>;

    #[abi(embed_v0)]
    impl MockContractImpl of super::IMockContract<ContractState> {
        fn test_call(ref self: ContractState) {
            assert(self.operator.is_call_allowed(), 'call not allowed');
        }
    }
}

fn setup_mock() -> (ContractAddress, ClassHash) {
    let contract = snforge_std::declare("mock_contract").unwrap_syscall().contract_class();
    let (addr, _) = contract.deploy(@array![]).unwrap_syscall();

    (addr, *contract.class_hash)
}

#[test]
fn test_operator_initialize() {
    let (contract_address, _) = setup_mock();

    let op_dispatcher = IOperatorDispatcher { contract_address };
    let mock_dispatcher = IMockContractDispatcher { contract_address };

    mock_dispatcher.test_call();

    // snforge_std::start_cheat_caller_address_global('OTHER'.try_into().unwrap());
    // op_dispatcher.change_mode(OperatorMode::Disabled);
}
