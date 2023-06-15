#[contract]
mod World {
    use array::{ArrayTrait, SpanTrait};
    use traits::Into;
    use option::OptionTrait;
    use box::BoxTrait;
    use serde::Serde;
    use starknet::{
        get_caller_address, get_contract_address, get_tx_info,
        contract_address::ContractAddressIntoFelt252, ClassHash, Zeroable, ContractAddress
    };

    use dojo_core::storage::{db::Database, query::{Query, QueryTrait}};
    use dojo_core::execution_context::Context;
    use dojo_core::auth::components::AuthRole;
    use dojo_core::auth::systems::Route;
    use dojo_core::interfaces::{
        IComponentLibraryDispatcher, IComponentDispatcherTrait, IExecutorDispatcher,
        IExecutorDispatcherTrait, ISystemLibraryDispatcher, ISystemDispatcherTrait
    };

    #[event]
    fn WorldSpawned(address: ContractAddress, caller: ContractAddress) {}

    #[event]
    fn ComponentRegistered(name: felt252, class_hash: ClassHash) {}

    #[event]
    fn SystemRegistered(name: felt252, class_hash: ClassHash) {}

    struct Storage {
        executor_dispatcher: IExecutorDispatcher,
        component_registry: LegacyMap::<felt252, ClassHash>,
        system_registry: LegacyMap::<felt252, ClassHash>,
        _execution_role: LegacyMap::<ContractAddress, felt252>,
        systems_for_execution: LegacyMap::<(ContractAddress, felt252), bool>,
        initialized: bool,
        nonce: usize,
    }

    const ADMIN: felt252 = 'sudo';

    #[constructor]
    fn constructor(executor: ContractAddress) {
        executor_dispatcher::write(IExecutorDispatcher { contract_address: executor });

        WorldSpawned(get_contract_address(), get_tx_info().unbox().account_contract_address);
    }


    /// Initialize the world with the routes that specify
    /// the permissions for each system to access components.
    /// ** This function can only be called once. **
    ///
    /// # Arguments
    ///
    /// * `routes` - An array of routes that specify the permissions for each system to access components.
    /// 
    /// # Example
    ///
    /// ```
    /// let mut route = ArrayTrait::new();
    /// let target_id = 'Bar'.into();
    /// let role_id = 'FooWriter'.into();
    /// let resource_id = 'Foo'.into();
    /// let r = Route { target_id, role_id, resource_id,  };
    #[external]
    fn initialize(routes: Array<Route>) {
        // Assert that the world has not been initialized
        assert(!initialized::read(), 'already initialized');

        // Get the RouteAuth system class hash
        let route_auth_class_hash = system_registry::read('RouteAuth'.into());

        // Loop through each route and handle the auth.
        // This grants the system the permission to specific components.
        let mut index = 0;
        loop {
            if index == routes.len() {
                break ();
            }

            // Serialize the route
            let mut calldata = ArrayTrait::new();
            let r = routes.at(index);
            r.serialize(ref calldata);

            // Call RouteAuth system via executor with the serialized route
            executor_dispatcher::read()
                .execute(route_auth_class_hash, AuthRole { id: ADMIN.into() }, calldata.span());

            index += 1;
        };

        // Set the initialized flag.
        initialized::write(true);
    }

    /// Check if system is authorized to write to the component
    ///
    /// # Arguments
    ///
    /// * `system` - The system that is attempting to write to the component
    /// * `component` - The component that is being written to
    /// * `execution_role` - The execution role of the system
    ///
    /// # Returns
    ///
    /// * `bool` - True if the system is authorized to write to the component, false otherwise
    #[view]
    fn is_authorized(
        system: felt252, component: felt252, execution_role: AuthRole
    ) -> bool {
        let is_authorized_class_hash = system_registry::read('IsAuthorized'.into());

        // If the world has been initialized, check the authorization.
        // World is initialized when WorldFactory::spawn is called
        if initialized::read() {
            // If component to be updated is AuthStatus or AuthRole, check if the caller account is Admin
            if component == 'AuthStatus'.into() | component == 'AuthRole'.into() {
                is_account_admin()
            } else {
                // Check if the system is authorized to write to the component
                let mut calldata = ArrayTrait::new();
                calldata.append(system.into()); // target_id
                calldata.append(component.into()); // resource_id

                // Call IsAuthorized system via executor with serialized system and component
                // If the system is authorized, the result will be non-zero
                let res = executor_dispatcher::read()
                    .execute(is_authorized_class_hash, execution_role, calldata.span());
                (*res[0]).is_non_zero()
            }
        } else {
            // If the world has not yet been initialized, all systems are authorized.
            // This is to allow the initial Admin role to be set
            true
        }
    }

    /// Check if the calling account has Admin role
    ///
    /// # Returns
    ///
    /// * `bool` - True if the calling account has Admin role, false otherwise
    #[view]
    fn is_account_admin() -> bool {
        let is_account_admin_class_hash = system_registry::read('IsAccountAdmin'.into());
        // Call IsAccountAdmin system via executor
        let mut calldata = ArrayTrait::new();
        let res = executor_dispatcher::read()
            .execute(is_account_admin_class_hash, AuthRole { id: ADMIN.into() }, calldata.span());
        (*res[0]).is_non_zero()
    }

    /// Register a component in the world. If the component is already registered,
    /// the implementation will be updated.
    ///
    /// # Arguments
    ///
    /// * `class_hash` - The class hash of the component to be registered
    #[external]
    fn register_component(class_hash: ClassHash) {
        let name = IComponentLibraryDispatcher { class_hash: class_hash }.name();
        // If component is already registered, validate permission to update.
        if component_registry::read(name).is_non_zero() {
            assert(is_account_admin(), 'only admin can update');
        }
        component_registry::write(name, class_hash);
        ComponentRegistered(name, class_hash);
    }

    /// Get the class hash of a registered component
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the component
    ///
    /// # Returns
    ///
    /// * `ClassHash` - The class hash of the component
    #[view]
    fn component(name: felt252) -> ClassHash {
        component_registry::read(name)
    }

    /// Register a system in the world. If the system is already registered,
    /// the implementation will be updated.
    ///
    /// # Arguments
    ///
    /// * `class_hash` - The class hash of the system to be registered
    #[external]
    fn register_system(class_hash: ClassHash) {
        let name = ISystemLibraryDispatcher { class_hash: class_hash }.name();
        // If system is already registered, validate permission to update.
        if system_registry::read(name).is_non_zero() {
            assert(is_account_admin(), 'only admin can update');
        }
        system_registry::write(name, class_hash);
        SystemRegistered(name, class_hash);
    }

    /// Get the class hash of a registered system
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the system
    ///
    /// # Returns
    ///
    /// * `ClassHash` - The class hash of the system
    #[view]
    fn system(name: felt252) -> ClassHash {
        system_registry::read(name)
    }

    /// Execute a system with the given calldata
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the system to be executed
    /// * `execute_calldata` - The calldata to be passed to the system
    ///
    /// # Returns
    ///
    /// * `Span<felt252>` - The result of the system execution
    #[external]
    fn execute(name: felt252, execute_calldata: Span<felt252>) -> Span<felt252> {
        // Get the class hash of the system to be executed
        let class_hash = system_registry::read(name);

        // Get execution role
        let role = execution_role();

        // Call the system via executor
        let res = executor_dispatcher::read()
            .execute(class_hash, AuthRole { id: role }, execute_calldata);

        res
    }

    /// Issue an autoincremented id to the caller.
    ///
    /// # Returns
    ///
    /// * `usize` - The autoincremented id
    #[external]
    fn uuid() -> usize {
        let current = nonce::read();
        nonce::write(current + 1);
        current
    }

    /// Set the component value for an entity
    ///
    /// # Arguments
    ///
    /// * `component` - The name of the component to be set
    /// * `query` - The query to be used to find the entity
    /// * `offset` - The offset of the component in the entity
    /// * `value` - The value to be set
    /// * `context` - The execution context of the system call
    #[external]
    fn set_entity(
        context: Context, component: felt252, query: Query, offset: u8, value: Span<felt252>
    ) {
        // Assert can only be called through the executor
        // This is to prevent system from writing to storage directly
        assert(
            get_caller_address() == executor_dispatcher::read().contract_address,
            'must be called thru executor'
        );

        // Fallback to default scoped authorization check if role is not set
        fallback_authorization_check(context.caller_account, context.caller_system, component);

        // Set the entity
        let table = query.table(component);
        let component_class_hash = component_registry::read(component);
        Database::set(component_class_hash, table, query, offset, value)
    }

    /// Delete a component from an entity
    ///
    /// # Arguments
    ///
    /// * `component` - The name of the component to be deleted
    /// * `query` - The query to be used to find the entity
    /// * `context` - The execution context of the system call
    #[external]
    fn delete_entity(context: Context, component: felt252, query: Query) {
        // Assert can only be called through the executor
        // This is to prevent system from writing to storage directly
        assert(
            get_caller_address() == executor_dispatcher::read().contract_address,
            'must be called thru executor'
        );

        // Fallback to default scoped authorization check if role is not set
        fallback_authorization_check(context.caller_account, context.caller_system, component);

        // Delete the entity
        let table = query.table(component);
        let component_class_hash = component_registry::read(component);
        let res = Database::del(component_class_hash, component.into(), query);
    }

    /// Get the component value for an entity
    ///
    /// # Arguments
    ///
    /// * `component` - The name of the component to be retrieved
    /// * `query` - The query to be used to find the entity
    /// * `offset` - The offset of the component in the entity
    ///
    /// # Returns
    ///
    /// * `Span<felt252>` - The value of the component
    #[view]
    fn entity(component: felt252, query: Query, offset: u8, length: usize) -> Span<felt252> {
        let class_hash = component_registry::read(component);
        let table = query.table(component);
        match Database::get(class_hash, table, query, offset, length) {
            Option::Some(res) => res,
            Option::None(_) => {
                ArrayTrait::new().span()
            }
        }
    }

    /// Returns entity IDs and entities that contain the component state.
    ///
    /// # Arguments
    ///
    /// * `component` - The name of the component to be retrieved
    /// * `partition` - The partition to be retrieved
    ///
    /// # Returns
    ///
    /// * `Span<felt252>` - The entity IDs
    /// * `Span<Span<felt252>>` - The entities
    #[view]
    fn entities(component: felt252, partition: felt252) -> (Span<felt252>, Span<Span<felt252>>) {
        let class_hash = component_registry::read(component);
        Database::all(class_hash, component.into(), partition)
    }

    /// Set the executor contract address
    ///
    /// # Arguments
    ///
    /// * `contract_address` - The contract address of the executor
    #[external]
    fn set_executor(contract_address: ContractAddress) {
        // Only Admin can set executor
        assert(is_account_admin(), 'only admin can set executor');
        executor_dispatcher::write(IExecutorDispatcher { contract_address: contract_address });
    }

    #[view]
    fn executor() -> ContractAddress {
        executor_dispatcher::read().contract_address
    }

    /// Validate that the role to be assumed has the required permissions
    /// then set the execution role
    /// 
    /// # Arguments
    ///
    /// * `role_id` - The role id to be assumed
    /// * `systems` - The systems to be validated
    #[external]
    fn assume_role(role_id: felt252, systems: Array<felt252>) {
        // Only Admin can set Admin role 
        let caller = get_tx_info().unbox().account_contract_address;
        if role_id == ADMIN.into() {
            assert(is_account_admin(), 'only admin can set Admin role');
        } else {
            let mut index = 0;
            let len = systems.len();

            // Loop through the systems to be validated
            loop {
                if index == len {
                    break ();
                }

                // Get the system's components
                let system = *systems[index];
                let components = system_components(system);

                let mut index_inner = 0;
                let len_inner = components.len();

                // Loop through each component
                loop {
                    if index_inner == len_inner {
                        break ();
                    }
                    let (component, write) = *components[index_inner];
                    if write {
                        // Validate that the role to be assumed has the required permissions
                        assert(
                            is_authorized(system, component, AuthRole { id: role_id }),
                            'role not authorized'
                        );
                    }
                    index_inner += 1;
                };
                // Set the system for execution
                systems_for_execution::write((caller, system), true);
                index += 1;
            };
        };
        // Set the execution role
        _execution_role::write(caller, role_id);
    }

    /// Clear the execution role and systems for execution
    ///
    /// # Arguments
    ///
    /// * `systems` - The systems to be cleared
    #[external]
    fn clear_role(systems: Array<felt252>) {
        // Clear the execution role
        let caller = get_tx_info().unbox().account_contract_address;
        _execution_role::write(caller, 0.into());

        // Clear systems for execution
        let mut index = 0;
        let len = systems.len();
        loop {
            if index == len {
                break ();
            }
            let system = *systems[index];
            systems_for_execution::write((caller, system), false);
            index += 1;
        };
    }

    /// Get the assumed execution role
    ///
    /// # Arguments
    ///
    /// # Returns
    ///
    /// * `felt252` - The role id of the system
    #[view]
    fn execution_role() -> felt252 {
        let caller = get_tx_info().unbox().account_contract_address;
        _execution_role::read(caller)
    }

    /// Get the component dependencies of a system
    ///
    /// # Arguments
    ///
    /// * `system` - The system to be retrieved
    ///
    /// # Returns
    ///
    /// * `Array<(felt252, bool)>` - The component dependencies of the system
    /// bool is true if the system is writing to the component
    #[view]
    fn system_components(system: felt252) -> Array<(felt252, bool)> {
        let class_hash = system_registry::read(system);
        ISystemLibraryDispatcher { class_hash }.dependencies()
    }

    /// Check if the system is part of the systems for execution
    ///
    /// # Arguments
    ///
    /// * `system` - The system to be retrieved
    ///
    /// # Returns
    ///
    /// * `bool` - True if the system is part of the systems for execution
    #[view]
    fn is_system_for_execution(system: felt252) -> bool {
        let caller = get_tx_info().unbox().account_contract_address;
        systems_for_execution::read((caller, system))
    }

    /// Internals

    /// If no role is set, check if the system's default scoped permission to write to the component is authorized
    ///
    /// # Arguments
    ///
    /// * `caller` - The caller account address
    /// * `system` - The system to be retrieved
    /// * `component` - The component to be retrieved
    fn fallback_authorization_check(
        caller: ContractAddress, system: felt252, component: felt252
    ) {
        // Get execution role
        let role = execution_role();

        // Validate authorization if role is not set
        // Otherwise, validate that the system is part of the systems for execution if role is not Admin
        if role.into() == 0 {
            // Validate the calling system has permission to write to the component
            assert(
                is_authorized(system, component, AuthRole { id: role }), 'system not authorized'
            );
        } else if role.into() != ADMIN {
            assert(systems_for_execution::read((caller, system)), 'system not for execution');
        };
    }
}

#[system]
mod LibraryCall {
    use dojo_core::serde::SpanSerde;

    fn execute(
        class_hash: starknet::ClassHash, entrypoint: felt252, calladata: Span<felt252>
    ) -> Span<felt252> {
        starknet::syscalls::library_call_syscall(class_hash, entrypoint, calladata).unwrap_syscall()
    }
}
