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
    fn initialize(routing: Array<Route>) {
        // Assert only the Admin role can initialize the world.
        let caller: u250  = get_caller_address().into();
        let role = entity('Role'.into(), QueryTrait::new_from_id(caller), 0_u8, 0_usize);
        assert(*role[0] == 'Admin', 'caller not admin');

        // Assert that the world has not been initialized
        assert(!initialized::read(), 'already initialized');

        let class_hash = system_registry::read('RouteAuth'.into());
        let mut index = 0;

        // Loop through each route and handle the auth.
        // This grants the system the permission to specific components.
        loop {
            if index == routing.len() {
                break ();
            }

            // Serialize the route
            let mut calldata = ArrayTrait::new();
            let r = routing.at(index);
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

    // Check if the world has been initialized.
    #[view]
    fn is_initialized() -> bool {
        initialized::read()
    }

    // Check if system is authorized to write to the component
    #[view]
    fn check_auth(system: ClassHash, component: ClassHash) -> bool {
        let class_hash = system_registry::read('Authorize'.into());
        let mut calldata = ArrayTrait::<felt252>::new();
        calldata.append(system.into()); // caller_id
        calldata.append(component.into()); // resource_id
        let res = IExecutorDispatcher {
            contract_address: executor::read()
        }.execute(class_hash, calldata.span());
        (*res[0]).is_non_zero()
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
        let system_class_hash = caller::read();
        let table = query.table(component);
        let component_class_hash = component_registry::read(component);

        // Validate the calling system has permission to write to the component
        assert_auth(system_class_hash, component_class_hash);
        Database::set(component_class_hash, table, query, offset, value)
    }

    #[external]
    fn delete_entity(component: ShortString, query: Query) {
        let system_class_hash = caller::read();
        let table = query.table(component);
        let component_class_hash = component_registry::read(component);

        // Validate the calling system has permission to write to the component
        assert_auth(system_class_hash, component_class_hash);
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

    // Internals

    // Assert that calling system has authorization to write to the component
    fn assert_auth(system: ClassHash, component: ClassHash) {
        // Get AuthorizeSystem ClassHash
        let authorize_class_hash = system_registry::read('Authorize'.into());

        // Assert only when world is initialized
        // This is so initial roles can be set before the world is initialized.
        if initialized::read() {
            let mut calldata = ArrayTrait::new();
            calldata.append(system.into()); // caller_id
            calldata.append(component.into()); // resource_id
            IExecutorDispatcher {
                contract_address: executor::read()
            }.execute(authorize_class_hash, calldata.span());
        }
    }
}
