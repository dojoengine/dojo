#[system]
mod spawn {
    use array::ArrayTrait;
    use box::BoxTrait;
    use traits::Into;
    use dojo::world::Context;

    use dojo_examples::components::Position;
    use dojo_examples::components::Vec2;
    use dojo_examples::components::Moves;
    use dojo_examples::components::Direction;

    fn execute(ctx: Context) {
        let position = get !(ctx.world, ctx.origin, (Position));
        set !(
            ctx.world,
            (
                Moves {
                    player: ctx.origin, remaining: 10, last_direction: Direction::None(())
                    }, Position {
                    player: ctx.origin, vec: Vec2 { x: position.vec.x + 10, y: position.vec.y + 10 }
                },
            )
        );
        return ();
    }
}

#[system]
mod move {
    use starknet::ContractAddress;
    use array::ArrayTrait;
    use box::BoxTrait;
    use traits::Into;
    use dojo::world::Context;

    use dojo_examples::components::Position;
    use dojo_examples::components::Moves;
    use dojo_examples::components::Direction;

    #[event]
    #[derive(Drop, starknet::Event)]
    enum Event {
        Moved: Moved, 
    }

    #[derive(Drop, starknet::Event)]
    struct Moved {
        address: ContractAddress,
        direction: Direction
    }

    fn execute(ctx: Context, direction: Direction) {
        let (mut position, mut moves) = get!(ctx.world, ctx.origin, (Position, Moves));
        moves.remaining -= 1;
        moves.last_direction = direction;
        let next = next_position(position, direction);
        set!(ctx.world, (moves, next));
        emit!(ctx.world, Moved { address: ctx.origin, direction });
        return ();
    }

    fn next_position(mut position: Position, direction: Direction) -> Position {
        match direction {
            Direction::None(()) => {
                return position;
            },
            Direction::Left(()) => {
                position.vec.x -= 1;
            },
            Direction::Right(()) => {
                position.vec.x += 1;
            },
            Direction::Up(()) => {
                position.vec.y -= 1;
            },
            Direction::Down(()) => {
                position.vec.y += 1;
            },
        };

        position
    }
}

// #[cfg(test)]
// mod tests {
//     use core::traits::Into;
//     use array::ArrayTrait;
//     use debug::PrintTrait;

//     use dojo::world::IWorldDispatcherTrait;
//     use dojo::database::schema::SchemaIntrospection;

//     use dojo::test_utils::spawn_test_world;

//     use dojo_examples::components::position;
//     use dojo_examples::components::Position;
//     use dojo_examples::components::moves;
//     use dojo_examples::components::Moves;
//     use dojo_examples::systems::spawn;
//     use dojo_examples::systems::move;


//     #[test]
//     #[available_gas(30000000)]
//     fn test_move() {
//         let caller = starknet::contract_address_const::<0x0>();

//         // components
//         let mut components = array::ArrayTrait::new();
//         components.append(position::TEST_CLASS_HASH);
//         components.append(moves::TEST_CLASS_HASH);
//         // components.append(dojo_erc::erc20::components::balance::TEST_CLASS_HASH);
//         // systems
//         let mut systems = array::ArrayTrait::new();
//         systems.append(spawn::TEST_CLASS_HASH);
//         systems.append(move::TEST_CLASS_HASH);

//         // deploy executor, world and register components/systems
//         let world = spawn_test_world(components, systems);

//         let spawn_call_data = array::ArrayTrait::new();
//         world.execute('spawn', spawn_call_data);

//         let mut move_calldata = array::ArrayTrait::new();
//         move_calldata.append(move::Direction::Right(()).into());
//         world.execute('move', move_calldata);
//         let mut keys = array::ArrayTrait::new();
//         keys.append(caller.into());

//         let moves = world.entity('Moves', keys.span(), 0, SchemaIntrospection::<Moves>::size(), array![8, 8].span());
//         assert(*moves[0] == 9, 'updated packed value is wrong');
//         assert(*moves[1] == 2, 'updated packed value is wrong');
//         let new_position = world
//             .entity('Position', keys.span(), 0, SchemaIntrospection::<Position>::size(), array![32, 32].span());
//         assert(*new_position[0] == 0xa0000000b, 'packed position x,y is wrong');
//         assert(*new_position[1] == 0, 'position value is wrong');
//     }
// }
