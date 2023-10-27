#[cfg(test)]
mod tests {
    use dojo::world::{IWorldDispatcherTrait, IWorldDispatcher};
    use dojo::test_utils::spawn_test_world;

    // project imports
    use dojo_examples::components::{position, Position};
    use dojo_examples::components::{moves, Moves};
    use dojo_examples::systems::spawn;
    use dojo_examples::systems::move;
    use dojo_examples::constants::OFFSET;

    #[event]
    use dojo_examples::events::{Event, Moved};


    // helper setup function
    // reuse this function for all tests
    fn setup_world() -> IWorldDispatcher {
        // components
        let mut components = array![position::TEST_CLASS_HASH, moves::TEST_CLASS_HASH];

        // systems
        let mut systems = array![spawn::TEST_CLASS_HASH, move::TEST_CLASS_HASH];

        // deploy executor, world and register components/systems
        spawn_test_world(components, systems)
    }

    #[test]
    #[available_gas(300000000)]
    fn test_move() {
        let world = setup_world();

        // spawn entity
        world.execute('spawn', array![]);

        // move entity
        world.execute('move', array![move::Direction::Right(()).into()]);

        // it is just the caller
        let caller = starknet::contract_address_const::<0x0>();

        // check moves
        let moves = get!(world, caller, (Moves));
        assert(moves.remaining == 99, 'moves is wrong');

        // check position
        let new_position = get!(world, caller, (Position));
        assert(new_position.x == (OFFSET + 1).try_into().unwrap(), 'position x is wrong');
        assert(new_position.y == OFFSET.try_into().unwrap(), 'position y is wrong');

        //check events

        // unpop world creation events
        let mut events_to_unpop = 1; // WorldSpawned
        events_to_unpop += 2; // 2x ComponentRegistered
        events_to_unpop += 2; // 2x SystemRegistered
        loop {
            if events_to_unpop == 0 {
                break;
            };

            starknet::testing::pop_log_raw(world.contract_address);
            events_to_unpop -= 1;
        };

        starknet::testing::pop_log_raw(world.contract_address); // unpop StoreSetRecord Moves
        starknet::testing::pop_log_raw(world.contract_address); // unpop StoreSetRecord Position
        // player spawns at x:OFFSET, y:OFFSET
        assert(
            @starknet::testing::pop_log(world.contract_address)
                .unwrap() == @Event::Moved(
                    Moved {
                        player: caller, x: OFFSET.try_into().unwrap(), y: OFFSET.try_into().unwrap()
                    }
                ),
            'invalid Moved event 0'
        );

        starknet::testing::pop_log_raw(world.contract_address); // unpop StoreSetRecord Moves
        starknet::testing::pop_log_raw(world.contract_address); // unpop StoreSetRecord Position
        // player move at x:OFFSET+1, y:OFFSET
        assert(
            @starknet::testing::pop_log(world.contract_address)
                .unwrap() == @Event::Moved(
                    Moved {
                        player: caller,
                        x: (OFFSET + 1).try_into().unwrap(),
                        y: OFFSET.try_into().unwrap()
                    }
                ),
            'invalid Moved event 1'
        );
    }
}
