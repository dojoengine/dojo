#[cfg(test)]
mod tests {
    use starknet::ContractAddress;
    use dojo::test_utils::spawn_test_world;
    use dojo_chess::components::{Game, game, GameTurn, game_turn, Square, square, PieceType};

    use dojo_chess::systems::initiate_system;
    use dojo_chess::systems::move_system;
    use array::ArrayTrait;
    use core::traits::Into;
    use dojo::world::IWorldDispatcherTrait;
    use core::array::SpanTrait;


    #[test]
    #[available_gas(3000000000000000)]
    fn integration() {
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
        systems.append(move_system::TEST_CLASS_HASH);
        let world = spawn_test_world(components, systems);

        // initiate
        let mut calldata = array::ArrayTrait::<core::felt252>::new();
        calldata.append(white.into());
        calldata.append(black.into());
        world.execute('initiate_system'.into(), calldata);

        let game_id =  pedersen::pedersen(white.into(), black.into());

        //White pawn is now in (0,1)
        let a2 = get!(world, (game_id, 0, 1), (Square));
        match a2.piece {
            Option::Some(piece) => {
                assert(piece == PieceType::WhitePawn, 'should be White Pawn in (0,1)');
            },
            Option::None(_) => assert(false, 'should have piece in (0,1)'),
        };

        //Black pawn is now in (1,6)
        let b7 = get!(world, (game_id, 1, 6), (Square));
        match b7.piece {
            Option::Some(piece) => {
                assert(piece == PieceType::BlackPawn, 'should be Black Pawn in (1,6)');
            },
            Option::None(_) => assert(false, 'should have piece in (1,6)'),
        };

        //Move White Pawn to (0,3)
        let mut move_calldata = array::ArrayTrait::<core::felt252>::new();
        move_calldata.append(0);
        move_calldata.append(1);
        move_calldata.append(0);
        move_calldata.append(3);
        move_calldata.append(white.into());
        move_calldata.append(game_id);
        world.execute('move_system'.into(), move_calldata);

        //White pawn is now in (0,3)
        let a4 = get!(world, (game_id, 0, 3), (Square));
        match a4.piece {
            Option::Some(piece) => {
                assert(piece == PieceType::WhitePawn, 'should be White Pawn in (0,3)');
            },
            Option::None(_) => assert(false, 'should have piece in (0,3)'),
        };

        //Move black Pawn to (1,4)
        let mut move_calldata = array::ArrayTrait::<core::felt252>::new();
        move_calldata.append(1);
        move_calldata.append(6);
        move_calldata.append(1);
        move_calldata.append(4);
        move_calldata.append(black.into());
        move_calldata.append(game_id);
        world.execute('move_system'.into(), move_calldata);

        //Black pawn is now in (1,4)
        let b5 = get!(world, (game_id, 1, 4), (Square));
        match b5.piece {
            Option::Some(piece) => {
                assert(piece == PieceType::BlackPawn, 'should be Black Pawn  in (1,4)');
            },
            Option::None(_) => assert(false, 'should have piece  in (1,4)'),
        };

        // Move White Pawn to (1,4)
        // Capture black pawn
        let mut move_calldata = array::ArrayTrait::<core::felt252>::new();
        move_calldata.append(0);
        move_calldata.append(3);
        move_calldata.append(1);
        move_calldata.append(4);
        move_calldata.append(white.into());
        move_calldata.append(game_id);
        world.execute('move_system'.into(), move_calldata);

        let b5 = get!(world, (game_id, 1, 4), (Square));
        match b5.piece {
            Option::Some(piece) => {
                assert(piece == PieceType::WhitePawn, 'should be WhitePawn  in (1,4)');
            },
            Option::None(_) => assert(false, 'should have piece in (1,4)'),
        };
    }
}
