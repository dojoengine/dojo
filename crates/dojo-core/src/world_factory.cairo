use array::ArrayTrait;

#[contract]
mod WorldFactory {
    use array::ArrayTrait;
    use option::OptionTrait;
    use traits::Into;

    use starknet::{ClassHash, ContractAddress, contract_address::ContractAddressIntoFelt252, syscalls::deploy_syscall, get_caller_address};

    use dojo_core::interfaces::{IWorldDispatcher, IWorldDispatcherTrait};
    use dojo_core::{string::ShortString, auth::systems::Route};

    struct Storage {
        world_class_hash: ClassHash,
        executor_address: ContractAddress,
        auth_components: LegacyMap::<usize, ClassHash>,
        auth_systems: LegacyMap::<usize, ClassHash>,
        auth_components_len: usize,
        auth_systems_len: usize,
    }

    #[event]
    fn WorldSpawned(address: ContractAddress) {}

    #[constructor]
    fn constructor(
        world_class_hash_: ClassHash,
        executor_address_: ContractAddress,
        auth_components_: Array<ClassHash>,
        auth_systems_: Array<ClassHash>
    ) {
        world_class_hash::write(world_class_hash_);
        executor_address::write(executor_address_);
        // Write auth components to storage through mapping
        let mut index = 0;
        let len = auth_components_.len();
        loop {
            if index == len {
                break ();
            }
            auth_components::write(index, *auth_components_.at(index));
            index += 1;
        };
        auth_components_len::write(len);

        // Write auth systems to storage through mapping
        let mut index = 0;
        let len = auth_systems_.len();
        loop {
            if index == len {
                break ();
            }
            auth_systems::write(index, *auth_systems_.at(index));
            index += 1;
        };
        auth_systems_len::write(len);
    }

    #[external]
    fn spawn(
        name: ShortString,
        components: Array<ClassHash>,
        systems: Array<ClassHash>,
        routes: Array<Route>,
    ) -> ContractAddress {
        // deploy world
        let mut world_constructor_calldata: Array<felt252> = ArrayTrait::new();
        world_constructor_calldata.append(name.into());
        world_constructor_calldata.append(executor_address::read().into());
        let world_class_hash = world_class_hash::read();
        let result = deploy_syscall(world_class_hash, 0, world_constructor_calldata.span(), true);
        let (world_address, _) = result.unwrap_syscall();
        let world = IWorldDispatcher { contract_address: world_address };

        // events
        WorldSpawned(world_address);

        // register default auth components and systems
        register_auth(world_address);

        // give deployer the Admin role
        let caller = get_caller_address();
        let mut grant_role_calldata: Array<felt252> = ArrayTrait::new();

        grant_role_calldata.append(caller.into()); // target_id
        grant_role_calldata.append('Admin'); // role_id
        world.execute('GrantAuthRole'.into(), grant_role_calldata.span());

        // register components
        let components_len = components.len();
        register_components(components, components_len, 0, world_address);

        // register systems
        let systems_len = systems.len();
        register_systems(systems, systems_len, 0, world_address);

        // initialize world by setting the auth routes
        world.initialize(routes);

        return world_address;
    }

    #[external]
    fn set_executor(executor_address_: ContractAddress) {
        executor_address::write(executor_address_);
    }

    #[external]
    fn set_world(class_hash: ClassHash) {
        world_class_hash::write(class_hash);
    }

    #[view]
    fn executor() -> ContractAddress {
        return executor_address::read();
    }

    #[view]
    fn world() -> ClassHash {
        return world_class_hash::read();
    }

    #[view]
    fn default_auth_components() -> Array<ClassHash> {
        let mut result: Array<ClassHash> = ArrayTrait::new();
        let len = auth_components_len::read();
        let mut index = 0;
        loop {
            if index == len {
                break ();
            }
            result.append(auth_components::read(index));
            index += 1;
        };
        result
    }

    #[view]
    fn default_auth_systems() -> Array<ClassHash> {
        let mut result: Array<ClassHash> = ArrayTrait::new();
        let len = auth_systems_len::read();
        let mut index = 0;
        loop {
            if index == len {
                break ();
            }
            result.append(auth_systems::read(index));
            index += 1;
        };
        result
    }

    fn register_components(
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
        return register_components(components, components_len, index + 1, world_address);
    }

    fn register_systems(
        systems: Array<ClassHash>, systems_len: usize, index: usize, world_address: ContractAddress
    ) {
        if (index == systems_len) {
            return ();
        }
        IWorldDispatcher { contract_address: world_address }.register_system(*systems.at(index));
        return register_systems(systems, systems_len, index + 1, world_address);
    }

    fn register_auth(world_address: ContractAddress) {
        // Register auth components
        let auth_components = default_auth_components();
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
        let auth_systems = default_auth_systems();
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
