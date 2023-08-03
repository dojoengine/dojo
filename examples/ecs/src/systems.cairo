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
            ctx.world,
            (
                Moves {
                    player: ctx.origin, remaining: 10
                    }, Position {
                    player: ctx.origin, x: 0, y: 0
                },
            )
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
        let (mut position, mut moves) = get !(ctx.world, ctx.origin, (Position, Moves));
        moves.remaining -= 1;
        let next = next_position(position, direction);
        set !(ctx.world, (moves, next));
        return ();
    }

    fn next_position(mut position: Position, direction: Direction) -> Position {
        match direction {
            Direction::Left(()) => {
                position.x -= 1;
            },
            Direction::Right(()) => {
                position.x += 1;
            },
            Direction::Up(()) => {
                position.y -= 1;
            },
            Direction::Down(()) => {
                position.y += 1;
            },
        };

        position
    }
}

#[cfg(test)]
mod tests {
    use core::traits::Into;
    use array::ArrayTrait;

    use dojo::world::IWorldDispatcherTrait;

    use dojo::test_utils::spawn_test_world;

    use dojo_examples::components::position;
    use dojo_examples::components::Position;
    use dojo_examples::components::moves;
    use dojo_examples::components::Moves;
    use dojo_examples::systems::spawn;
    use dojo_examples::systems::move;

    #[test]
    #[available_gas(30000000)]
    fn test_move() {
        let caller = starknet::contract_address_const::<0x0>();

        // components
        let mut components = array::ArrayTrait::new();
        components.append(position::TEST_CLASS_HASH);
        components.append(moves::TEST_CLASS_HASH);
        // systems
        let mut systems = array::ArrayTrait::new();
        systems.append(spawn::TEST_CLASS_HASH);
        systems.append(move::TEST_CLASS_HASH);

        // deploy executor, world and register components/systems
        let world = spawn_test_world(components, systems);

        let spawn_call_data = array::ArrayTrait::new();
        world.execute('spawn', spawn_call_data.span());

        let mut move_calldata = array::ArrayTrait::new();
        move_calldata.append(move::Direction::Right(()).into());
        world.execute('move', move_calldata.span());

        let mut keys = array::ArrayTrait::new();
        keys.append(caller.into());

        let moves = world.entity('Moves', keys.span(), 0, dojo::SerdeLen::<Moves>::len());
        assert(*moves[0] == 9, 'moves is wrong');
        let new_position = world
            .entity('Position', keys.span(), 0, dojo::SerdeLen::<Position>::len());
        assert(*new_position[0] == 1, 'position x is wrong');
        assert(*new_position[1] == 0, 'position y is wrong');
    }
}
