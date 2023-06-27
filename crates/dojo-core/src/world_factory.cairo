use array::ArrayTrait;

#[starknet::contract]
mod WorldFactory {
    use array::ArrayTrait;
    use option::OptionTrait;
    use traits::Into;

    use starknet::{
        ClassHash, ContractAddress, contract_address::ContractAddressIntoFelt252,
        syscalls::deploy_syscall, get_caller_address
    };

    use dojo_core::interfaces::{IWorldDispatcher, IWorldDispatcherTrait};
    use dojo_core::auth::systems::Route;

    #[storage]
    struct Storage {
        world_class_hash: ClassHash,
        executor_address: ContractAddress,
        auth_components: LegacyMap::<usize, ClassHash>,
        auth_systems: LegacyMap::<usize, ClassHash>,
        auth_components_len: usize,
        auth_systems_len: usize,
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
        ref self: ContractState,
        world_class_hash_: ClassHash,
        executor_address_: ContractAddress,
        auth_components_: Array<ClassHash>,
        auth_systems_: Array<ClassHash>
    ) {
        self.world_class_hash.write(world_class_hash_);
        self.executor_address.write(executor_address_);
        // Write auth components to storage through mapping
        let mut index = 0;
        let len = auth_components_.len();
        loop {
            if index == len {
                break ();
            }
            self.auth_components.write(index, *auth_components_.at(index));
            index += 1;
        };
        self.auth_components_len.write(len);

        // Write auth systems to storage through mapping
        let mut index = 0;
        let len = auth_systems_.len();
        loop {
            if index == len {
                break ();
            }
            self.auth_systems.write(index, *auth_systems_.at(index));
            index += 1;
        };
        self.auth_systems_len.write(len);
    }

    #[external(v0)]
    fn spawn(
        ref self: ContractState, components: Array<ClassHash>, systems: Array<ClassHash>, routes: Array<Route>, 
    ) -> ContractAddress {
        // deploy world
        let mut world_constructor_calldata: Array<felt252> = ArrayTrait::new();
        world_constructor_calldata.append(self.executor_address.read().into());
        let world_class_hash = self.world_class_hash.read();
        let result = deploy_syscall(world_class_hash, 0, world_constructor_calldata.span(), true);
        let (world_address, _) = result.unwrap_syscall();
        let world = IWorldDispatcher { contract_address: world_address };

        // events
        self.emit(WorldSpawned { address: world_address });

        // register default auth components and systems
        register_auth(@self, world_address);

        // give deployer the Admin role
        let caller = get_caller_address();
        let mut grant_role_calldata: Array<felt252> = ArrayTrait::new();

        grant_role_calldata.append(caller.into()); // target_id
        grant_role_calldata.append(dojo_core::world::World::ADMIN); // role_id
        world.execute('GrantAuthRole'.into(), grant_role_calldata.span());

        // register components
        let components_len = components.len();
        register_components(@self, components, components_len, 0, world_address);

        // register systems
        let systems_len = systems.len();
        register_systems(@self, systems, systems_len, 0, world_address);

        // initialize world by setting the auth routes
        world.initialize(routes);

        return world_address;
    }

    #[external(v0)]
    fn set_executor(ref self: ContractState, executor_address_: ContractAddress) {
        self.executor_address.write(executor_address_);
    }

    #[external(v0)]
    fn set_world(ref self: ContractState, class_hash: ClassHash) {
        self.world_class_hash.write(class_hash);
    }

    #[external(v0)]
    fn executor(self: @ContractState) -> ContractAddress {
        return self.executor_address.read();
    }

    #[external(v0)]
    fn world(self: @ContractState) -> ClassHash {
        return self.world_class_hash.read();
    }

    #[external(v0)]
    fn default_auth_components(self: @ContractState) -> Array<ClassHash> {
        let mut result: Array<ClassHash> = ArrayTrait::new();
        let len = self.auth_components_len.read();
        let mut index = 0;
        loop {
            if index == len {
                break ();
            }
            result.append(self.auth_components.read(index));
            index += 1;
        };
        result
    }

    #[external(v0)]
    fn default_auth_systems(self: @ContractState) -> Array<ClassHash> {
        let mut result: Array<ClassHash> = ArrayTrait::new();
        let len = self.auth_systems_len.read();
        let mut index = 0;
        loop {
            if index == len {
                break ();
            }
            result.append(self.auth_systems.read(index));
            index += 1;
        };
        result
    }

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

    fn register_systems(
        self: @ContractState, systems: Array<ClassHash>, systems_len: usize, index: usize, world_address: ContractAddress
    ) {
        if (index == systems_len) {
            return ();
        }
        IWorldDispatcher { contract_address: world_address }.register_system(*systems.at(index));
        return register_systems(self, systems, systems_len, index + 1, world_address);
    }

    fn register_auth(self: @ContractState, world_address: ContractAddress) {
        // Register auth components
        let auth_components = default_auth_components(self);
        let mut index = 0;
        loop {
            if index == auth_components.len() {
                break ();
            }
            IWorldDispatcher {
                contract_address: world_address
            }.register_component(*auth_components.at(index));
            index += 1;
        };

        // Register auth systems
        let auth_systems = default_auth_systems(self);
        let mut index = 0;
        loop {
            if index == auth_systems.len() {
                break ();
            }
            IWorldDispatcher {
                contract_address: world_address
            }.register_system(*auth_systems.at(index));
            index += 1;
        };
    }
}
