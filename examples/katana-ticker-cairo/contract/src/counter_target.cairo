#[starknet::interface]
trait ICounterTarget<TContractState> {
    fn tick(ref self: TContractState);
    fn get_counter(self: @TContractState) -> u256;
}

#[starknet::contract]
mod CounterTarget {
    use starknet::{ContractAddress, get_caller_address};

    #[storage]
    struct Storage {
        ticker: ContractAddress,
        counter: u256
    }

    #[event]
    #[derive(Drop, starknet::Event)]
    enum Event {
        Tick: Tick,
    }
    #[derive(Drop, starknet::Event)]
    struct Tick {
        counter: u256,
    }

    #[constructor]
    fn constructor(ref self: ContractState, ticker: ContractAddress) {
        self.ticker.write(ticker);
        self.counter.write(0);
    }

    #[external(v0)]
    impl CounterTarget of super::ICounterTarget<ContractState> {
        fn tick(ref self: ContractState) {
            assert(self.ticker.read() == get_caller_address(), 'Not ticker');
            self.counter.write(self.counter.read() + 1);
            self.emit(Event::Tick(Tick{counter: self.counter.read()}));
        }

        fn get_counter(self: @ContractState) -> u256 {
            self.counter.read()
        }
    }
}
