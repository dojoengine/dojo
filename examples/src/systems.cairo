#[system]
mod Spawn {
    use array::ArrayTrait;
    use traits::Into;   

    use dojo_examples::components::Position;
    use dojo_examples::components::Moves;

    fn execute() {
        let caller = starknet::get_caller_address();
        let player = commands::set_entity(caller.into(), (
            Moves { remaining: 10_u8 },
            Position { x: 0_u32, y: 0_u32 },
        ));
        return ();
    }
}

#[system]
mod Move {
    use array::ArrayTrait;
    use traits::Into;

    use dojo_examples::components::Position;
    use dojo_examples::components::Moves;

    // TODO: Use enum once serde is derivable
    // left: 0, right: 1, up: 2, down: 3
    fn execute(direction: felt252) {
        let caller = starknet::get_caller_address();
        let (position, moves) = commands::<Position, Moves>::entity(caller.into());
        let next = next_position(position, direction);
        let uh = commands::set_entity(caller.into(), (
            Moves { remaining: moves.remaining - 1_u8 },
            Position { x: next.x, y: next.y },
        ));
        return ();
    }

    fn next_position(position: Position, direction: felt252) -> Position {
        // TODO: Use match once supported
        // error: Only match zero (match ... { 0 => ..., _ => ... }) is currently supported.
        if direction == 0 {
            Position { x: position.x - 1_u32, y: position.y }
        } else if direction == 1 {
            Position { x: position.x + 1_u32, y: position.y }
        } else if direction == 2 {
            Position { x: position.x, y: position.y - 1_u32 }
        } else if direction == 3 {
            Position { x: position.x, y: position.y + 1_u32 }
        } else {
            position
        }
    }
}


mod tests {
    use core::traits::Into;
    use array::ArrayTrait;

    use dojo_core::interfaces::IWorldDispatcherTrait;
    use dojo_core::test_utils::spawn_test_world;

    use dojo_examples::components::PositionComponent;
    use dojo_examples::components::MovesComponent;
    use dojo_examples::systems::SpawnSystem;
    use dojo_examples::systems::MoveSystem;

    #[test]
    #[available_gas(30000000)]
    fn test_move() {
        // components
        let mut components = array::ArrayTrait::<felt252>::new();
        components.append(PositionComponent::TEST_CLASS_HASH);
        components.append(MovesComponent::TEST_CLASS_HASH);
        // systems
        let mut systems = array::ArrayTrait::<felt252>::new();
        systems.append(SpawnSystem::TEST_CLASS_HASH);
        systems.append(MoveSystem::TEST_CLASS_HASH);

        // deploy executor, world and register components/systems
        let world = spawn_test_world(components, systems);
    
        let spawn_call_data = array::ArrayTrait::<felt252>::new();
        world.execute('Spawn'.into(), spawn_call_data.span());

        let mut move_calldata = array::ArrayTrait::<felt252>::new();
        move_calldata.append(1);
        world.execute('Move'.into(), move_calldata.span());

        let world_address = world.contract_address;

        let moves = world.entity('Moves'.into(), world_address.into(), 0_u8, 0_usize);
        assert(*moves[0] == 9, 'moves is wrong');

        let new_position = world.entity('Position'.into(), world_address.into(), 0_u8, 0_usize);
        assert(*new_position[0] == 1, 'position x is wrong');
        assert(*new_position[1] == 0, 'position y is wrong');
    }

}

