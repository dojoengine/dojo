use starknet::ContractAddress;

#[starknet::interface]
trait ITarget<TContractState> {
    fn tick(ref self: TContractState);
}

#[starknet::interface]
trait ITicker<TContractState> {
    fn apply_tick(self: @TContractState);
    fn set_target(ref self: TContractState, target: ContractAddress);
    fn get_depositor(self: @TContractState) -> ContractAddress;
    fn get_operator(self: @TContractState) -> ContractAddress;
}

#[starknet::contract]
mod Ticker {
    use super::{ITargetDispatcher, ITargetDispatcherTrait};
    use starknet::{ContractAddress, get_caller_address};
    use starknet::contract_address::ContractAddressZeroable;
    use zeroable::Zeroable;

    #[storage]
    struct Storage {
        depositor: ContractAddress,
        operator: ContractAddress,
        target: ContractAddress,
    }

    #[constructor]
    fn constructor(ref self: ContractState, operator: ContractAddress) {
        self.depositor.write(get_caller_address());
        self.operator.write(operator);
        self.target.write(ContractAddressZeroable::zero());
    }

    #[generate_trait]
    impl InternalFunctions of InternalFunctionsTrait {
        fn only_depositor(self: @ContractState) {
            assert(get_caller_address() == self.depositor.read(), 'Not depositor');
        }

        fn only_allowed(self: @ContractState) {
            let caller = get_caller_address();
            assert(
                caller == self.depositor.read() || caller == self.operator.read(), 'Not allowed'
            );
        }
    }

    #[external(v0)]
    impl Ticker of super::ITicker<ContractState> {
        fn apply_tick(self: @ContractState) {
            self.only_depositor();
            if (self.target.read().is_non_zero()) {
                ITargetDispatcher { contract_address: self.target.read() }.tick();
            }
        }

        fn set_target(ref self: ContractState, target: ContractAddress) {
            self.only_allowed();
            self.target.write(target);
        }

        fn get_depositor(self: @ContractState) -> ContractAddress {
            self.depositor.read()
        }

        fn get_operator(self: @ContractState) -> ContractAddress {
            self.operator.read()
        }
    }
}
