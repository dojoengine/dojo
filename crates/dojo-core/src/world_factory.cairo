use array::ArrayTrait;

use starknet::{ClassHash, ContractAddress};

#[starknet::interface]
trait IWorldFactory<T> {
    fn set_world(ref self: T, class_hash: ClassHash);
    fn set_executor(ref self: T, executor_address: ContractAddress);
    fn spawn(
        ref self: T, components: Array<ClassHash>, systems: Array<ClassHash>,
    ) -> ContractAddress;
    fn world(self: @T) -> ClassHash;
    fn executor(self: @T) -> ContractAddress;
}

#[starknet::contract]
mod world_factory {
    use array::ArrayTrait;
    use option::OptionTrait;
    use traits::Into;

    use starknet::{
        ClassHash, ContractAddress, contract_address::ContractAddressIntoFelt252,
        syscalls::deploy_syscall, get_caller_address
    };

    use dojo::world::{IWorldDispatcher, IWorldDispatcherTrait};

    use super::IWorldFactory;

    #[storage]
    struct Storage {
        world_class_hash: ClassHash,
        executor_address: ContractAddress,
        custom_executor_path: Option<felt252>,
    }

    #[event]
    #[derive(Drop, starknet::Event)]
    enum Event {
        WorldSpawned: WorldSpawned
    }

    #[derive(Drop, starknet::Event)]
    struct WorldSpawned {
        address: ContractAddress
    }

    #[constructor]
    fn constructor(
        ref self: ContractState, world_class_hash_: ClassHash, executor_address_: ContractAddress,custom_executor_path_: Option<felt252>
    ) {
        self.world_class_hash.write(world_class_hash_);
        self.executor_address.write(executor_address_);

        self.custom_executor_path.write(custom_executor_path_);
    }

    #[external(v0)]
    impl WorldFactory of IWorldFactory<ContractState> {
        /// Spawns a new world with the given components and systems.
        ///
        /// # Arguments
        ///
        /// * `components` - The components to be registered.
        /// * `systems` - The systems to be registered.
        ///
        /// # Returns
        ///
        /// The address of the deployed world.
        fn spawn(
            ref self: ContractState, components: Array<ClassHash>, systems: Array<ClassHash>,
        ) -> ContractAddress {
            // deploy world
            let mut world_constructor_calldata: Array<felt252> = ArrayTrait::new();
            world_constructor_calldata.append(self.executor_address.read().into());
            let world_class_hash = self.world_class_hash.read();
            let result = deploy_syscall(
                world_class_hash, 0, world_constructor_calldata.span(), true
            );
            let (world_address, _) = result.unwrap_syscall();
            let world = IWorldDispatcher { contract_address: world_address };

            // events
            self.emit(WorldSpawned { address: world_address });

            // register components
            let components_len = components.len();
            register_components(@self, components, components_len, 0, world_address);

            // register systems
            let systems_len = systems.len();
            register_systems(@self, systems, systems_len, 0, world_address);

            return world_address;
        }

        /// Sets the executor address.
        ///
        /// # Arguments
        ///
        /// * `executor_address` - The address of the executor.
        fn set_executor(ref self: ContractState, executor_address: ContractAddress) {
            self.executor_address.write(executor_address);
        }

        /// Sets the class hash for the world.
        ///
        /// # Arguments
        ///
        /// * `class_hash` - The class hash of world.
        fn set_world(ref self: ContractState, class_hash: ClassHash) {
            self.world_class_hash.write(class_hash);
        }

        /// Gets the executor contract address.
        ///
        /// # Returns
        ///
        /// * `ContractAddress` - The address of the executor contract.
        fn executor(self: @ContractState) -> ContractAddress {
            return self.executor_address.read();
        }

        /// Gets the world class hash.
        ///
        /// # Returns
        ///
        /// * `ClassHash` - The class hash of the world.
        fn world(self: @ContractState) -> ClassHash {
            return self.world_class_hash.read();
        }
    }

    /// Registers all the given components in the world at the given address.
    ///
    /// # Arguments
    ///
    /// * `components` - The components to be registered.
    /// * `components_len` - The number of components to register.
    /// * `index` - The index where to start the registration in the components list.
    /// * `world_address` - The address of the world where components are registered.
    fn register_components(
        self: @ContractState,
        components: Array<ClassHash>,
        components_len: usize,
        index: usize,
        world_address: ContractAddress
    ) {
        if (index == components_len) {
            return ();
        }
        IWorldDispatcher {
            contract_address: world_address
        }.register_component(*components.at(index));
        return register_components(self, components, components_len, index + 1, world_address);
    }

    /// Registers all the given systems in the world at the given address.
    ///
    /// # Arguments
    ///
    /// * `systems` - The systems to be registered.
    /// * `systems_len` - The number of systems to register.
    /// * `index` - The index where to start the registration in the system list.
    /// * `world_address` - The address of the world where systems are registered.
    fn register_systems(
        self: @ContractState,
        systems: Array<ClassHash>,
        systems_len: usize,
        index: usize,
        world_address: ContractAddress
    ) {
        if (index == systems_len) {
            return ();
        }
        IWorldDispatcher { contract_address: world_address }.register_system(*systems.at(index));
        return register_systems(self, systems, systems_len, index + 1, world_address);
    }
}
