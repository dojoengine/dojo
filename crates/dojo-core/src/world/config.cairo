use starknet::ContractAddress;

pub mod errors {
    pub const INVALID_CALLER: felt252 = 'Config: not owner or operator';
    pub const ALREADY_REGISTERED: felt252 = 'Config: already operator';
    pub const NOT_OPERATOR: felt252 = 'Config: not operator';
}

#[starknet::interface]
pub trait IConfig<T> {
    /// Sets the information of the program that generates the
    /// state transition trace (namely DojoOS).
    ///
    /// # Arguments
    ///
    /// * `program_hash` - The program hash.
    /// * `config_hash` - The program's config hash.
    fn set_differ_program_hash(ref self: T, program_hash: felt252);
    fn set_merger_program_hash(ref self: T, program_hash: felt252);

    /// Gets the information of the program that generates the
    /// state transition trace (namely DojoOS).
    ///
    /// # Returns
    ///
    /// The program hash and it's configuration hash.
    fn get_differ_program_hash(self: @T) -> felt252;
    fn get_merger_program_hash(self: @T) -> felt252;

    /// Sets the facts registry contract address, which is already
    /// initialized with the verifier information.
    ///
    /// # Arguments
    ///
    /// * `address` - The facts registry contract's address.
    fn set_facts_registry(ref self: T, address: ContractAddress);

    /// Gets the facts registry contract address.
    ///
    /// # Returns
    ///
    /// The contract address of the facts registry.
    fn get_facts_registry(self: @T) -> ContractAddress;
}

#[starknet::component]
pub mod Config {
    use starknet::ContractAddress;
    use starknet::get_caller_address;
    use starknet::event::EventEmitter;
    use starknet::storage::{StoragePointerReadAccess, StoragePointerWriteAccess};

    use super::errors;
    use super::IConfig;

    #[event]
    #[derive(Drop, starknet::Event, Debug, PartialEq)]
    pub enum Event {
        DifferProgramHashUpdate: DifferProgramHashUpdate,
        MergerProgramHashUpdate: MergerProgramHashUpdate,
        FactsRegistryUpdate: FactsRegistryUpdate
    }

    #[derive(Drop, starknet::Event, Debug, PartialEq)]
    pub struct DifferProgramHashUpdate {
        pub program_hash: felt252,
    }

    #[derive(Drop, starknet::Event, Debug, PartialEq)]
    pub struct MergerProgramHashUpdate {
        pub program_hash: felt252,
    }

    #[derive(Drop, starknet::Event, Debug, PartialEq)]
    pub struct FactsRegistryUpdate {
        pub address: ContractAddress
    }

    #[storage]
    pub struct Storage {
        differ_program_hash: felt252,
        merger_program_hash: felt252,
        facts_registry: ContractAddress,
        owner: ContractAddress
    }

    #[generate_trait]
    pub impl InternalImpl<
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

