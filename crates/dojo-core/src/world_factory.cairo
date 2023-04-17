use array::ArrayTrait;

#[contract]
mod WorldFactory {
    use dojo_core::interfaces::IWorldDispatcher;
    use dojo_core::interfaces::IWorldDispatcherTrait;
    use starknet::ContractAddress;
    use starknet::ClassHash;
    use starknet::syscalls::deploy_syscall;
    use array::ArrayTrait;
    use traits::Into;
    use option::OptionTrait;
    use starknet::contract_address::ContractAddressIntoFelt252;

    struct Storage {
        executor_class_hash: ClassHash,
        world_class_hash: ClassHash,
        executor_contracts: LegacyMap::<felt252, ContractAddress>,
        world_contracts: LegacyMap::<felt252, ContractAddress>,
    }

    #[event]
    fn ContractsDeployed(name: felt252, executor: ContractAddress, world: ContractAddress) {}

    #[constructor]
    fn constructor(executor_class_hash_: ClassHash, world_class_hash_: ClassHash) {
        executor_class_hash::write(executor_class_hash_);
        world_class_hash::write(world_class_hash_);
    }

    #[external]
    fn spawn(name: felt252, components: Array::<ClassHash>, systems: Array::<ClassHash>) {
        // assert name not taken
        assert(world_contracts::read(name).into() == 0, 'name already taken');

        // deploy executor
        let mut executor_constructor_calldata: Array<felt252> = ArrayTrait::new();
        let executor_class_hash = executor_class_hash::read();
        let result = deploy_syscall(
            executor_class_hash, 0, executor_constructor_calldata.span(), true
        );
        let (executor_address, _) = result.unwrap_syscall();
        executor_contracts::write(name, executor_address);

        // deploy world
        let mut world_constructor_calldata: Array<felt252> = ArrayTrait::new();
        world_constructor_calldata.append(name);
        world_constructor_calldata.append(executor_address.into());
        let world_class_hash = world_class_hash::read();
        let result = deploy_syscall(world_class_hash, 0, world_constructor_calldata.span(), true);
        let (world_address, _) = result.unwrap_syscall();
        world_contracts::write(name, world_address);

        // events
        ContractsDeployed(name, executor_address, world_address);

        // register components
        let components_len = components.len();
        register_components(components, components_len, 0_usize, world_address);

        // register systems
        let systems_len = systems.len();
        register_systems(systems, systems_len, 0_usize, world_address);
    }

    #[external]
    fn set_executor(class_hash: ClassHash) {
        executor_class_hash::write(class_hash);
    }

    #[external]
    fn set_world(class_hash: ClassHash) {
        world_class_hash::write(class_hash);
    }

    #[view]
    fn get_executor_class_hash() -> ClassHash {
        return executor_class_hash::read();
    }

    #[view]
    fn get_world_class_hash() -> ClassHash {
        return world_class_hash::read();
    }

    fn register_components(
        components: Array<ClassHash>,
        components_len: usize,
        index: usize,
        world_address: ContractAddress
    ) {
        match gas::withdraw_gas() {
            Option::Some(_) => {},
            Option::None(_) => {
                let mut data = ArrayTrait::new();
                data.append('Out of gas');
                panic(data);
            },
        }
        if (index == components_len) {
            return ();
        }
        IWorldDispatcher {
            contract_address: world_address
        }.register_component(*components.at(index));
        return register_components(components, components_len, index + 1_usize, world_address);
    }

    fn register_systems(
        systems: Array<ClassHash>, systems_len: usize, index: usize, world_address: ContractAddress
    ) {
        match gas::withdraw_gas() {
            Option::Some(_) => {},
            Option::None(_) => {
                let mut data = ArrayTrait::new();
                data.append('Out of gas');
                panic(data);
            },
        }
        if (index == systems_len) {
            return ();
        }
        IWorldDispatcher { contract_address: world_address }.register_system(*systems.at(index));
        return register_systems(systems, systems_len, index + 1_usize, world_address);
    }
}

#[test]
#[available_gas(2000000)]
fn test_constructor() {
    WorldFactory::constructor(starknet::class_hash_const::<0x420>(), starknet::class_hash_const::<0x69>());
    let executor_class_hash = WorldFactory::get_executor_class_hash();
    assert(executor_class_hash == starknet::class_hash_const::<0x420>(), 'wrong executor class hash');
    let world_class_hash = WorldFactory::get_world_class_hash();
    assert(world_class_hash == starknet::class_hash_const::<0x69>(), 'wrong world class hash');
}