extern type Query<T>;

#[abi]
trait IWorld {
    fn lookup(from: felt) -> felt;
}

#[contract]
mod MoveSystem {
    use starknet::call_contract_syscall;
    use starknet::contract_address_try_from_felt;
    use array::ArrayTrait;
    use option::OptionTrait;
    use option::OptionTraitImpl;

    struct Storage {
        world_address: felt, 
    }

    #[external]
    fn initialize(world_addr: felt) {
        let world = world_address::read();
        assert(world == 0, 'MoveSystem: Already initialized.');
        world_address::write(world_addr);
    }

    #[external]
    fn execute() {
        let world = world_address::read();
        assert(world != 0, 'MoveSystem: Not initialized.');

        let world_address = contract_address_try_from_felt(world).unwrap();
        let position_ids = super::IWorldDispatcher::lookup(
            world_address, 0x1a42f66f387f576f66678aa85131976ee602be23c3d1bc7597fdeb1e40b9687
        );
        let position_ids = super::IWorldDispatcher::lookup(
            world_address, 0x1a42f66f387f576f66678aa85131976ee602be23c3d1bc7597fdeb1e40b9687
        );
        let world_address = contract_address_try_from_felt(world).unwrap();
        let position_ids = super::IWorldDispatcher::lookup(
            world_address, 0x737c494e6fbf007e7b84f73bc84f202746ae6a51bc789d374fe8290ce2a8ab
        );
        let health_ids = super::IWorldDispatcher::lookup(
            world_address, 0x737c494e6fbf007e7b84f73bc84f202746ae6a51bc789d374fe8290ce2a8ab
        );

        return ();
    }
}
