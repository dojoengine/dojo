#[system]
mod spawn {
    use array::ArrayTrait;
    use box::BoxTrait;
    use traits::Into;
    use dojo::world::Context;

    use dojo_examples::components::Position;
    use dojo_examples::components::Moves;

    fn execute(ctx: Context) {
        set !(
            ctx.world, ctx.origin.into(), (Moves { remaining: 10 }, Position { x: 0, y: 0 }, )
        );
        return ();
    }
}

#[system]
mod move {
    use array::ArrayTrait;
    use box::BoxTrait;
    use traits::Into;
    use dojo::world::Context;

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

    fn execute(ctx: Context, direction: Direction) {
        let (position, moves) = get !(ctx.world, ctx.origin.into(), (Position, Moves));
        let next = next_position(position, direction);
        set !(
            ctx.world,
            ctx.origin.into(),
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

#[cfg(test)]
mod tests {
    use core::traits::Into;
    use array::ArrayTrait;

    use dojo::interfaces::IWorldDispatcherTrait;

    use dojo::test_utils::spawn_test_world;

    use dojo_examples::components::PositionComponent;
    use dojo_examples::components::MovesComponent;
    use dojo_examples::systems::spawn;
    use dojo_examples::systems::move;

    #[test]
    #[available_gas(30000000)]
    fn test_move() {
        let caller = starknet::contract_address_const::<0x0>();

        // components
        let mut components = array::ArrayTrait::new();
        components.append(PositionComponent::TEST_CLASS_HASH);
        components.append(MovesComponent::TEST_CLASS_HASH);
        // systems
        let mut systems = array::ArrayTrait::new();
        systems.append(spawn::TEST_CLASS_HASH);
        systems.append(move::TEST_CLASS_HASH);

        // deploy executor, world and register components/systems
        let world = spawn_test_world(components, systems);

        let spawn_call_data = array::ArrayTrait::new();
        world.execute('spawn'.into(), spawn_call_data.span());

        let mut move_calldata = array::ArrayTrait::new();
        move_calldata.append(move::Direction::Right(()).into());
        world.execute('move'.into(), move_calldata.span());

        let moves = world.entity('Moves'.into(), caller.into(), 0, 0);
        assert(*moves[0] == 9, 'moves is wrong');
        let new_position = world.entity('Position'.into(), caller.into(), 0, 0);
        assert(*new_position[0] == 1, 'position x is wrong');
        assert(*new_position[1] == 0, 'position y is wrong');
    }
}
