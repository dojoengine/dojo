#[contract]
mod World {
    use array::ArrayTrait;
    use array::SpanTrait;
    use traits::Into;
    use option::OptionTrait;
    use box::BoxTrait;
    use serde::Serde;
    use starknet::get_caller_address;
    use starknet::get_contract_address;
    use starknet::get_tx_info;
    use starknet::contract_address::ContractAddressIntoFelt252;
    use starknet::ClassHash;
    use starknet::Zeroable;
    use starknet::ContractAddress;

    use dojo_core::storage::query::Query;
    use dojo_core::storage::query::QueryTrait;
    use dojo_core::storage::db::Database;
    use dojo_core::integer::u250;
    use dojo_core::integer::ContractAddressIntoU250;
    use dojo_core::string::ShortString;
    use dojo_core::auth::systems::Route;

    use dojo_core::interfaces::IComponentLibraryDispatcher;
    use dojo_core::interfaces::IComponentDispatcherTrait;
    use dojo_core::interfaces::IExecutorDispatcher;
    use dojo_core::interfaces::IExecutorDispatcherTrait;
    use dojo_core::interfaces::ISystemLibraryDispatcher;
    use dojo_core::interfaces::ISystemDispatcherTrait;

    #[event]
    fn WorldSpawned(address: ContractAddress, caller: ContractAddress, name: ShortString) {}

    #[event]
    fn ComponentRegistered(name: ShortString, class_hash: ClassHash) {}

    #[event]
    fn SystemRegistered(name: ShortString, class_hash: ClassHash) {}

    struct Storage {
        caller: ClassHash,
        executor: ContractAddress,
        component_registry: LegacyMap::<ShortString, ClassHash>,
        system_registry: LegacyMap::<ShortString, ClassHash>,
        initialized: bool,
        nonce: usize,
    }

    #[constructor]
    fn constructor(name: ShortString, executor_: ContractAddress) {
        executor::write(executor_);

        WorldSpawned(get_contract_address(), get_tx_info().unbox().account_contract_address, name);
    }

    // Initialize the world with the routes that specify
    // the permissions for each system to access components.
    #[external]
    fn initialize(routes: Array<Route>) {
        // Assert that the world has not been initialized
        assert(!initialized::read(), 'already initialized');

        let class_hash = system_registry::read('RouteAuth'.into());
        let mut index = 0;

        // Loop through each route and handle the auth.
        // This grants the system the permission to specific components.
        loop {
            if index == routes.len() {
                break ();
            }

            // Serialize the route
            let mut calldata = ArrayTrait::new();
            let r = routes.at(index);
            r.serialize(ref calldata);
 
            // Call RouteAuth system via executor with the serialized route
            IExecutorDispatcher {
                contract_address: executor::read()
            }.execute(class_hash, calldata.span());

            index += 1;
        };

        // Set the initialized flag.
        initialized::write(true);
    }

    // Check if system is authorized to write to the component
    #[view]
    fn is_authorized(system: ClassHash, component: ClassHash) -> bool {
        let authorize_class_hash = system_registry::read('Authorize'.into());

        // If the world has been initialized, check the authorization.
        // World is initialized when WorldFactory::spawn is called
        if initialized::read() {
            let mut calldata = ArrayTrait::<felt252>::new();
            let system_name = ISystemLibraryDispatcher { class_hash: system }.name();
            let component_name = IComponentLibraryDispatcher { class_hash: component }.name();
            calldata.append(system_name.into()); // caller_id
            calldata.append(component_name.into()); // resource_id

            // Call Authorize system via executor with serialized system and component
            // If the system is authorized, the result will be non-zero
            let res = IExecutorDispatcher {
                contract_address: executor::read()
            }.execute(authorize_class_hash, calldata.span());
            (*res[0]).is_non_zero()
        } else {
            // If the world has not been initialized, all systems are authorized.
            // This is to allow the initial roles and auth routes to be set
            true
        }
    }

    // Register a component in the world. If the component is already registered,
    // the implementation will be updated.
    #[external]
    fn register_component(class_hash: ClassHash) {
        let name = IComponentLibraryDispatcher { class_hash: class_hash }.name();
        // TODO: If component is already registered, vaildate permission to update.
        component_registry::write(name, class_hash);
        ComponentRegistered(name, class_hash);
    }

    #[view]
    fn component(name: ShortString) -> ClassHash {
        component_registry::read(name)
    }

    // Register a system in the world. If the system is already registered,
    // the implementation will be updated.
    #[external]
    fn register_system(class_hash: ClassHash) {
        let name = ISystemLibraryDispatcher { class_hash: class_hash }.name();
        // TODO: If system is already registered, vaildate permission to update.
        system_registry::write(name, class_hash);
        SystemRegistered(name, class_hash);
    }

    #[view]
    fn system(name: ShortString) -> ClassHash {
        system_registry::read(name)
    }

    #[external]
    fn execute(name: ShortString, execute_calldata: Span<felt252>) -> Span<felt252> {
        let class_hash = system_registry::read(name);
        caller::write(class_hash);

        let res = IExecutorDispatcher {
            contract_address: executor::read()
        }.execute(class_hash, execute_calldata);

        caller::write(starknet::class_hash_const::<0x0>());
        res
    }

    // Issue an autoincremented id to the caller.
    #[external]
    fn uuid() -> usize {
        let next = nonce::read();
        nonce::write(next + 1);
        return next;
    }

    #[external]
    fn set_entity(component: ShortString, query: Query, offset: u8, value: Span<felt252>) {
        // Assert can only be called through the executor
        // This is to prevent system from writing to storage directly
        assert(get_caller_address() == executor::read(), 'must be called thru executor');

        let system_class_hash = caller::read();
        let table = query.table(component);
        let component_class_hash = component_registry::read(component);

        // Validate the calling system has permission to write to the component
        assert(is_authorized(system_class_hash, component_class_hash), 'system not authorized');
        Database::set(component_class_hash, table, query, offset, value)
    }

    #[external]
    fn delete_entity(component: ShortString, query: Query) {
        // Assert can only be called through the executor
        // This is to prevent system from writing to storage directly
        assert(get_caller_address() == executor::read(), 'must be called thru executor');

        let system_class_hash = caller::read();
        let table = query.table(component);
        let component_class_hash = component_registry::read(component);

        // Validate the calling system has permission to write to the component
        assert(is_authorized(system_class_hash, component_class_hash), 'system not authorized');
        let res = Database::del(system_class_hash, component.into(), query);
    }

    #[view]
    fn entity(component: ShortString, query: Query, offset: u8, length: usize) -> Span<felt252> {
        let class_hash = component_registry::read(component);
        match Database::get(class_hash, component.into(), query, offset, length) {
            Option::Some(res) => res,
            Option::None(_) => {
                ArrayTrait::<felt252>::new().span()
            }
        }
    }

    // Returns entities that contain the component state.
    #[view]
    fn entities(component: ShortString, partition: u250) -> Array::<u250> {
        Database::all(component.into(), partition)
    }

    #[external]
    fn set_executor(contract_address: ContractAddress) {
        executor::write(contract_address);
    }
}
