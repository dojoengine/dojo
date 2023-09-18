#[system]
mod initiate_system {
    use array::ArrayTrait;
    use traits::Into;
    use dojo::world::Context;
    use starknet::ContractAddress;
    use dojo_chess::components::{Color, Square, PieceType, Game, GameTurn};

    fn execute(ctx: Context, white_address: ContractAddress, black_address: ContractAddress) {
        let game_id = pedersen::pedersen(white_address.into(), black_address.into());
        set!(
            ctx.world,
            (
                Game {
                    game_id: game_id,
                    winner: Option::None(()),
                    white: white_address,
                    black: black_address,
                    }, GameTurn {
                    game_id: game_id, turn: Color::White(()), 
                },
            )
        );

        set!(
            ctx.world,
            (Square { game_id: game_id, x: 0, y: 0, piece: Option::Some(PieceType::WhiteRook) })
        );

        set!(
            ctx.world,
            (Square { game_id: game_id, x: 0, y: 1, piece: Option::Some(PieceType::WhitePawn) })
        );

        set!(
            ctx.world,
            (Square { game_id: game_id, x: 1, y: 6, piece: Option::Some(PieceType::BlackPawn) })
        );

        set!(
            ctx.world,
            (Square { game_id: game_id, x: 1, y: 0, piece: Option::Some(PieceType::WhiteKnight) })
        );
    }
}

#[cfg(test)]
mod tests {
    use starknet::ContractAddress;
    use dojo::test_utils::spawn_test_world;
    use dojo_chess::components::{Game, game, GameTurn, game_turn, Square, square, PieceType};

    use dojo_chess::systems::initiate_system;
    use array::ArrayTrait;
    use core::traits::Into;
    use dojo::world::IWorldDispatcherTrait;
    use core::array::SpanTrait;

    #[test]
    #[available_gas(3000000000000000)]
    fn test_initiate() {
        let white = starknet::contract_address_const::<0x01>();
        let black = starknet::contract_address_const::<0x02>();

        // components
        let mut components = array::ArrayTrait::new();
        components.append(game::TEST_CLASS_HASH);
        components.append(game_turn::TEST_CLASS_HASH);
        components.append(square::TEST_CLASS_HASH);

        //systems
        let mut systems = array::ArrayTrait::new();
        systems.append(initiate_system::TEST_CLASS_HASH);
        let world = spawn_test_world(components, systems);

        let mut calldata = array::ArrayTrait::<core::felt252>::new();
        calldata.append(white.into());
        calldata.append(black.into());
        world.execute('initiate_system'.into(), calldata);

        let game_id = pedersen::pedersen(white.into(), black.into());

        //get game
        let game = get!(world, (game_id), (Game));
        assert(game.white == white, 'white address is incorrect');
        assert(game.black == black, 'black address is incorrect');

        //get a1 square
        let a1 = get!(world, (game_id, 0, 0), (Square));
        match a1.piece {
            Option::Some(piece) => {
                assert(piece == PieceType::WhiteRook, 'should be White Rook');
            },
            Option::None(_) => assert(false, 'should have piece'),
        };
    }
}
