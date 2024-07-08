mod errors {
    const INVALID_CALLER: felt252 = 'Config: not owner or operator';
    const ALREADY_REGISTERED: felt252 = 'Config: already operator';
    const NOT_OPERATOR: felt252 = 'Config: not operator';
}

#[starknet::component]
mod Config {
    use dojo::config::interface::IConfig;
    use starknet::ContractAddress;
    use super::errors;
    use starknet::get_caller_address;
    use starknet::event::EventEmitter;

    #[event]
    #[derive(Drop, starknet::Event, Debug, PartialEq)]
    pub enum Event {
        DifferProgramHashUpdate: DifferProgramHashUpdate,
        MergerProgramHashUpdate: MergerProgramHashUpdate,
        FactsRegistryUpdate: FactsRegistryUpdate
    }

    #[derive(Drop, starknet::Event, Debug, PartialEq)]
    pub struct DifferProgramHashUpdate {
        program_hash: felt252,
    }

    #[derive(Drop, starknet::Event, Debug, PartialEq)]
    pub struct MergerProgramHashUpdate {
        program_hash: felt252,
    }

    #[derive(Drop, starknet::Event, Debug, PartialEq)]
    pub struct FactsRegistryUpdate {
        address: ContractAddress
    }

    #[storage]
    struct Storage {
        differ_program_hash: felt252,
        merger_program_hash: felt252,
        facts_registry: ContractAddress,
        owner: ContractAddress
    }

    #[generate_trait]
    impl InternalImpl<
        TContractState, +HasComponent<TContractState>
    > of InternalTrait<TContractState> {
        fn initializer(ref self: ComponentState<TContractState>, owner: ContractAddress) {
            self.owner.write(owner);
        }
    }

    #[embeddable_as(ConfigImpl)]
    impl Config<
        TContractState, +HasComponent<TContractState>
    > of IConfig<ComponentState<TContractState>> {
        fn set_differ_program_hash(
            ref self: ComponentState<TContractState>, program_hash: felt252
        ) {
            assert(get_caller_address() == self.owner.read(), errors::INVALID_CALLER);
            self.differ_program_hash.write(program_hash);
            self.emit(DifferProgramHashUpdate { program_hash });
        }

        fn set_merger_program_hash(
            ref self: ComponentState<TContractState>, program_hash: felt252
        ) {
            assert(get_caller_address() == self.owner.read(), errors::INVALID_CALLER);
            self.merger_program_hash.write(program_hash);
            self.emit(MergerProgramHashUpdate { program_hash });
        }

        fn get_differ_program_hash(self: @ComponentState<TContractState>) -> felt252 {
            self.differ_program_hash.read()
        }

        fn get_merger_program_hash(self: @ComponentState<TContractState>) -> felt252 {
            self.merger_program_hash.read()
        }

        fn set_facts_registry(ref self: ComponentState<TContractState>, address: ContractAddress) {
            assert(get_caller_address() == self.owner.read(), errors::INVALID_CALLER);
            self.facts_registry.write(address);
            self.emit(FactsRegistryUpdate { address: address });
        }

        fn get_facts_registry(self: @ComponentState<TContractState>) -> ContractAddress {
            self.facts_registry.read()
        }
    }
}

