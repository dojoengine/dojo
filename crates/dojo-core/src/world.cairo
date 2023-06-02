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
    use dojo_core::integer::{u250, ContractAddressIntoU250};
    use dojo_core::{string::ShortString, auth::systems::Route};
    use dojo_core::interfaces::{
        IComponentLibraryDispatcher, IComponentDispatcherTrait, IExecutorDispatcher,
        IExecutorDispatcherTrait, ISystemLibraryDispatcher, ISystemDispatcherTrait
    };

    #[event]
    fn WorldSpawned(address: ContractAddress, caller: ContractAddress, name: ShortString) {}

    #[event]
    fn ComponentRegistered(name: ShortString, class_hash: ClassHash) {}

    #[event]
    fn SystemRegistered(name: ShortString, class_hash: ClassHash) {}

    struct Storage {
        executor: ContractAddress,
        component_registry: LegacyMap::<ShortString, ClassHash>,
        system_registry: LegacyMap::<ShortString, ClassHash>,
        _execution_role: LegacyMap::<ShortString, u250>,
        initialized: bool,
        nonce: usize,
    }

    #[constructor]
    fn constructor(name: ShortString, executor_: ContractAddress) {
        executor::write(executor_);

        WorldSpawned(get_contract_address(), get_tx_info().unbox().account_contract_address, name);
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
            IExecutorDispatcher {
                contract_address: executor::read()
            }.execute(route_auth_class_hash, AuthRole { id: 'Admin'.into() }, calldata.span());

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
        system: ShortString, component: ShortString, execution_role: AuthRole
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
                let res = IExecutorDispatcher {
                    contract_address: executor::read()
                }.execute(is_authorized_class_hash, execution_role, calldata.span());
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
        let res = IExecutorDispatcher {
            contract_address: executor::read()
        }.execute(is_account_admin_class_hash, AuthRole { id: 'Admin'.into() }, calldata.span());
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
    fn component(name: ShortString) -> ClassHash {
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
    fn system(name: ShortString) -> ClassHash {
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
    fn execute(name: ShortString, execute_calldata: Span<felt252>) -> Span<felt252> {
        // Get the class hash of the system to be executed
        let class_hash = system_registry::read(name);

        // Get execution role
        let role = _execution_role::read(name);

        // Call the system via executor
        let res = IExecutorDispatcher {
            contract_address: executor::read()
        }.execute(class_hash, AuthRole { id: role }, execute_calldata);

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
        component: ShortString, query: Query, offset: u8, value: Span<felt252>, context: Context
    ) {
        // Assert can only be called through the executor
        // This is to prevent system from writing to storage directly
        assert(get_caller_address() == executor::read(), 'must be called thru executor');

        // Get execution role
        let role = _execution_role::read(component);

        // Validate the calling system has permission to write to the component
        assert(
            is_authorized(context.caller_system, component, AuthRole { id: role }),
            'system not authorized'
        );

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
    fn delete_entity(component: ShortString, query: Query, context: Context) {
        // Assert can only be called through the executor
        // This is to prevent system from writing to storage directly
        assert(get_caller_address() == executor::read(), 'must be called thru executor');

        // Get execution role
        let role = _execution_role::read(component);

        // Validate the calling system has permission to write to the component
        assert(
            is_authorized(context.caller_system, component, AuthRole { id: role }),
            'system not authorized'
        );

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
    fn entity(component: ShortString, query: Query, offset: u8, length: usize) -> Span<felt252> {
        let class_hash = component_registry::read(component);
        match Database::get(class_hash, component.into(), query, offset, length) {
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
    /// * `Span<u250>` - The entity IDs
    /// * `Span<Span<felt252>>` - The entities
    #[view]
    fn entities(component: ShortString, partition: u250) -> (Span<u250>, Span<Span<felt252>>) {
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
        executor::write(contract_address);
    }

    /// Set the execution role for a system
    ///
    /// # Arguments
    ///
    /// * `system` - The name of the system
    /// * `role_id` - The role id of the system
    #[external]
    fn set_execution_role(system: ShortString, role_id: u250) {
        // Only Admin can set Admin role 
        if role_id == 'Admin'.into() {
            assert(is_account_admin(), 'only admin can set Admin role');
        }
        _execution_role::write(system, role_id);
    }

    /// Get the execution role for a system
    ///
    /// # Arguments
    ///
    /// * `system` - The name of the system
    ///
    /// # Returns
    ///
    /// * `u250` - The role id of the system
    #[view]
    fn execution_role(system: ShortString) -> u250 {
        _execution_role::read(system)
    }
}
