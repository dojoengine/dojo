use dojo::utils::operator::OperatorMode;
use dojo::utils::{IOperatorDispatcher, IOperatorDispatcherTrait};
use snforge_std::{ContractClassTrait, DeclareResultTrait, start_cheat_block_timestamp_global};
use starknet::{ContractAddress, SyscallResultTrait};

const OTHER: ContractAddress = 'OTHER'.try_into().unwrap();
const OPERATOR: ContractAddress = 'OPERATOR'.try_into().unwrap();

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

fn setup_mock() -> (IOperatorDispatcher, IMockContractDispatcher) {
    let contract = snforge_std::declare("mock_contract").unwrap_syscall().contract_class();
    let (addr, _) = contract.deploy(@array![]).unwrap_syscall();

    (
        IOperatorDispatcher { contract_address: addr },
        IMockContractDispatcher { contract_address: addr },
    )
}

#[test]
fn test_operator_initialize_and_owner() {
    // Default mode, operator mode is disabled.
    let (op_dispatcher, mock_dispatcher) = setup_mock();

    mock_dispatcher.test_call();

    // Switch to expiry mode.
    start_cheat_block_timestamp_global(100);
    op_dispatcher.change_mode(OperatorMode::ExpireAt(200));

    op_dispatcher.grant_operator(OPERATOR);

    snforge_std::start_cheat_caller_address_global(OPERATOR);
    mock_dispatcher.test_call();

    // Switch to never expire mode.
    snforge_std::start_cheat_caller_address_global(starknet::get_contract_address());
    op_dispatcher.change_mode(OperatorMode::NeverExpire);

    snforge_std::start_cheat_caller_address_global(OPERATOR);
    mock_dispatcher.test_call();
}

#[test]
fn test_operator_expire_at() {
    let (op_dispatcher, mock_dispatcher) = setup_mock();

    op_dispatcher.change_mode(OperatorMode::ExpireAt(100));
    start_cheat_block_timestamp_global(101);

    // Can be called without operator being valid, since operator mode is expired.
    mock_dispatcher.test_call();
}

#[test]
#[should_panic(expected: 'call not allowed')]
fn test_invalid_operator() {
    let (op_dispatcher, mock_dispatcher) = setup_mock();

    op_dispatcher.change_mode(OperatorMode::NeverExpire);

    snforge_std::start_cheat_caller_address_global(OTHER);
    mock_dispatcher.test_call();
}

#[test]
#[should_panic(expected: 'caller is not owner')]
fn test_not_owner_change_mode() {
    let (op_dispatcher, _) = setup_mock();

    snforge_std::start_cheat_caller_address_global(OTHER);

    op_dispatcher.change_mode(OperatorMode::NeverExpire);
}

#[test]
#[should_panic(expected: 'caller is not owner')]
fn test_not_owner_grant_operator() {
    let (op_dispatcher, _) = setup_mock();

    snforge_std::start_cheat_caller_address_global(OTHER);

    op_dispatcher.grant_operator(OPERATOR);
}

#[test]
#[should_panic(expected: 'caller is not owner')]
fn test_not_owner_revoke_operator() {
    let (op_dispatcher, _) = setup_mock();

    snforge_std::start_cheat_caller_address_global(OTHER);

    op_dispatcher.revoke_operator(OPERATOR);
}
