#[contract]
mod World {
    use array::ArrayTrait;
    use array::SpanTrait;
    use traits::Into;
    use starknet::get_caller_address;
    use starknet::get_contract_address;
    use starknet::ClassHash;
    use starknet::ContractAddress;

    use dojo_core::storage::query::Query;
    use dojo_core::storage::query::QueryTrait;
    use dojo_core::storage::db::Database;
    use dojo_core::integer::u250;
    use dojo_core::string::ShortString;

    use dojo_core::interfaces::IComponentLibraryDispatcher;
    use dojo_core::interfaces::IComponentDispatcherTrait;
    use dojo_core::interfaces::IExecutorDispatcher;
    use dojo_core::interfaces::IExecutorDispatcherTrait;
    use dojo_core::interfaces::ISystemLibraryDispatcher;
    use dojo_core::interfaces::ISystemDispatcherTrait;

    #[event]
    fn WorldSpawned(address: ContractAddress, name: ShortString) {}

    #[event]
    fn ComponentRegistered(name: ShortString, class_hash: ClassHash) {}

    #[event]
    fn SystemRegistered(name: ShortString, class_hash: ClassHash) {}

    struct Storage {
        caller: ClassHash,
        executor: ContractAddress,
        component_registry: LegacyMap::<ShortString, ClassHash>,
        system_registry: LegacyMap::<ShortString, ClassHash>,
        nonce: usize,
    }

    // Give deployer the default admin role.
    #[constructor]
    fn constructor(name: ShortString, executor_: ContractAddress) {
        executor::write(executor_);

        WorldSpawned(get_contract_address(), name);
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

        // TODO: verify executor has permission to write

        let class_hash = component_registry::read(component);
        Database::set(class_hash, table, query, offset, value)
    }

    #[external]
    fn delete_entity(component: ShortString, query: Query) {
        let class_hash = caller::read();
        let res = Database::del(class_hash, component.into(), query);
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
