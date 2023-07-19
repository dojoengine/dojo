#[cfg(test)]
mod ticker_tests {
    use ticker::ticker::{ITicker, ITickerDispatcher, ITickerDispatcherTrait, Ticker};
    use starknet::{deploy_syscall, ContractAddress, get_caller_address, contract_address_const};
    use starknet::testing::{set_caller_address, set_contract_address};
    use option::OptionTrait;
    use array::ArrayTrait;
    use traits::{Into, TryInto};
    use result::ResultTrait;
    use serde::Serde;

    use ticker::tests::test_counter_target::{
        ICounterTarget, ICounterTargetDispatcher, ICounterTargetDispatcherTrait, CounterTarget
    };

    fn fakeCaller(address: ContractAddress) {
        set_caller_address(address);
        set_contract_address(address);
    }

    // Deploy the Ticker contract and return (TICKER_ADDRESS, DEPOSITOR_ADDRESS, OPERATOR_ADDRESS)
    // Also set caller AND contract address to depositor
    fn setup() -> (ContractAddress, ContractAddress, ContractAddress) {
        let depositor: ContractAddress = contract_address_const::<1>();
        let operator: ContractAddress = contract_address_const::<2>();

        // Set depositor as default caller
        fakeCaller(depositor);

        let mut calldata = ArrayTrait::new();
        operator.serialize(ref calldata);

        let (address0, _) = deploy_syscall(
            Ticker::TEST_CLASS_HASH.try_into().unwrap(), 0, calldata.span(), false
        )
            .unwrap();
        let ticker = ITickerDispatcher { contract_address: address0 };

        (address0, depositor, operator)
    }

    fn deploy_counter_target(depositor: ContractAddress, address: ContractAddress) -> ContractAddress {
        fakeCaller(address);

        let mut calldata = ArrayTrait::new();
        depositor.serialize(ref calldata);

        let (contract_address, _) = deploy_syscall(
            CounterTarget::TEST_CLASS_HASH.try_into().unwrap(), 0, calldata.span(), false
        )
            .unwrap();
        contract_address
    }

    #[test]
    #[available_gas(2000000000)]
    fn test_deploy() {
        let (ticker_address, depositor, operator) = setup();
        let ticker = ITickerDispatcher { contract_address: ticker_address };

        assert(depositor == get_caller_address(), 'Caller is not depositor');
        assert(depositor == ticker.get_depositor(), 'Wrong depositor');
        assert(operator == ticker.get_operator(), 'Wrong operator');
    }

    #[test]
    #[available_gas(2000000000)]
    fn test_set_target() {
        let (ticker_address, depositor, operator) = setup();
        let ticker = ITickerDispatcher { contract_address: ticker_address };

        fakeCaller(operator);
        let target = deploy_counter_target(depositor, operator);

        ticker.set_target(target);
    }

    #[test]
    #[available_gas(2000000000)]
    fn test_tick_as_depositor() {
        let (ticker_address, depositor, operator) = setup();
        let ticker = ITickerDispatcher { contract_address: ticker_address };

        ticker.apply_tick();
    }

    #[test]
    #[available_gas(2000000000)]
    #[should_panic]
    fn test_tick_as_operator() {
        let (ticker_address, depositor, operator) = setup();
        let ticker = ITickerDispatcher { contract_address: ticker_address };

        fakeCaller(operator);
        ticker.apply_tick();
    }

    #[test]
    #[available_gas(2000000000)]
    #[should_panic]
    fn test_tick_as_not_allowed() {
        let (ticker_address, depositor, operator) = setup();
        let ticker = ITickerDispatcher { contract_address: ticker_address };

        let unallowed = contract_address_const::<3>();
        fakeCaller(unallowed);
        ticker.apply_tick();
    }

    #[test]
    #[available_gas(2000000000)]
    fn test_counter_tick() {
        let (ticker_address, depositor, operator) = setup();
        let ticker = ITickerDispatcher { contract_address: ticker_address };

        let target = deploy_counter_target(depositor, depositor);

        fakeCaller(operator);
        ticker.set_target(target);

        let counter_target = ICounterTargetDispatcher { contract_address: target };

        fakeCaller(depositor);

        let mut i: u256 = 0;
        loop {
            if i > 10 {
                break;
            }
            assert(counter_target.get_counter() == i, 'Counter before tick wrong');
            ticker.apply_tick();
            assert(counter_target.get_counter() == i + 1, 'Counter did not tick');

            i += 1;
        }
    }
}
