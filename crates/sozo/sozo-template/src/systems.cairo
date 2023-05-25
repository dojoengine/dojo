#[system]
mod Spawn {
    use array::ArrayTrait;
    use traits::Into;

    use dojo_examples::components::Position;
    use dojo_examples::components::Moves;

    fn execute() {
        let caller = starknet::get_caller_address();
        let player = commands::set_entity(
            caller.into(), (Moves { remaining: 10 }, Position { x: 0, y: 0 }, )
        );
        return ();
    }
}

#[system]
mod Move {
    use array::ArrayTrait;
    use traits::Into;

    use dojo_examples::components::Position;
    use dojo_examples::components::Moves;

    #[derive(Serde, Drop)]
    enum Direction {
        Left: (),
        Right: (),
        Up: (),
        Down: (),
    }

    impl DirectionIntoFelt252 of Into<Direction, felt252> {
        fn into(self: Direction) -> felt252 {
            match self {
                Direction::Left(()) => 0,
                Direction::Right(()) => 1,
                Direction::Up(()) => 2,
                Direction::Down(()) => 3,
            }
        }
    }

    fn execute(direction: Direction) {
        let caller = starknet::get_caller_address();
        let (position, moves) = commands::<Position, Moves>::entity(caller.into());
        let next = next_position(position, direction);
        let uh = commands::set_entity(
            caller.into(),
            (Moves { remaining: moves.remaining - 1 }, Position { x: next.x, y: next.y }, )
        );
        return ();
    }

    fn next_position(position: Position, direction: Direction) -> Position {
        match direction {
            Direction::Left(()) => {
                Position { x: position.x - 1, y: position.y }
            },
            Direction::Right(()) => {
                Position { x: position.x + 1, y: position.y }
            },
            Direction::Up(()) => {
                Position { x: position.x, y: position.y - 1 }
            },
            Direction::Down(()) => {
                Position { x: position.x, y: position.y + 1 }
            },
        }
    }
}

mod tests {
    use core::traits::Into;
    use array::ArrayTrait;

    use dojo_core::auth::systems::{Route, RouteTrait};
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
        let mut components = array::ArrayTrait::new();
        components.append(PositionComponent::TEST_CLASS_HASH);
        components.append(MovesComponent::TEST_CLASS_HASH);
        // systems
        let mut systems = array::ArrayTrait::new();
        systems.append(SpawnSystem::TEST_CLASS_HASH);
        systems.append(MoveSystem::TEST_CLASS_HASH);
        // routes
        let mut routes = array::ArrayTrait::new();
        routes.append(
            RouteTrait::new(
                'Move'.into(), // target_id
                'MovesWriter'.into(), // role_id
                'Moves'.into(), // resource_id
            )
        );
        routes.append(
            RouteTrait::new(
                'Move'.into(), // target_id
                'PositionWriter'.into(), // role_id
                'Position'.into(), // resource_id
            )
        );
        routes.append(
            RouteTrait::new(
                'Spawn'.into(), // target_id
                'MovesWriter'.into(), // role_id
                'Moves'.into(), // resource_id
            )
        );
        routes.append(
            RouteTrait::new(
                'Spawn'.into(), // target_id
                'PositionWriter'.into(), // role_id
                'Position'.into(), // resource_id
            )
        );

        // deploy executor, world and register components/systems
        let world = spawn_test_world(components, systems, routes);

        let spawn_call_data = array::ArrayTrait::new();
        world.execute('Spawn'.into(), spawn_call_data.span());

        let mut move_calldata = array::ArrayTrait::new();
        move_calldata.append(MoveSystem::Direction::Right(()).into());
        world.execute('Move'.into(), move_calldata.span());

        let world_address = world.contract_address;

        let moves = world.entity('Moves'.into(), world_address.into(), 0, 0);
        assert(*moves[0] == 9, 'moves is wrong');

        let new_position = world.entity('Position'.into(), world_address.into(), 0, 0);
        assert(*new_position[0] == 1, 'position x is wrong');
        assert(*new_position[1] == 0, 'position y is wrong');
    }
}
