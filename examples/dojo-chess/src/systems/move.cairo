#[system]
mod move_system {
    use array::ArrayTrait;
    use traits::Into;
    use dojo::world::Context;
    use debug::PrintTrait;
    use starknet::ContractAddress;
    use dojo_chess::components::{Color, Square, PieceType, Game, GameTurn};

    fn execute(
        ctx: Context,
        curr_position: (u32, u32),
        next_position: (u32, u32),
        caller: ContractAddress,
        game_id: felt252
    ) {
        let (current_x, current_y) = curr_position;
        let (next_x, next_y) = next_position;
        current_x.print();
        current_y.print();

        next_x.print();
        next_y.print();

        let mut current_square = get!(ctx.world, (game_id, current_x, current_y), (Square));

        // check if next_position is out of board or not
        assert(is_out_of_board(next_position), 'Should be inside board');

        // check if this is the right piece type move
        assert(
            is_right_piece_move(current_square.piece, curr_position, next_position),
            'Should be right piece move'
        );
        let target_piece = current_square.piece;
        // make current_square piece none and move piece to next_square 
        current_square.piece = Option::None(());
        let mut next_square = get!(ctx.world, (game_id, next_x, next_y), (Square));

        // check the piece already in next_suqare
        let maybe_next_square_piece = next_square.piece;
        match maybe_next_square_piece {
            Option::Some(maybe_piece) => {
                if is_piece_is_mine(maybe_piece) {
                    panic(array!['Already same color piece exist'])
                } else {
                    // occupy the piece
                    next_square.piece = target_piece;
                }
            },
            //if not exist, then just move the original piece
            Option::None(_) => {
                next_square.piece = target_piece;
            },
        };

        set!(ctx.world, (next_square));
        set!(ctx.world, (current_square));
    }

    fn is_piece_is_mine(maybe_piece: PieceType) -> bool {
        false
    }

    fn is_correct_turn(maybe_piece: PieceType, caller: ContractAddress, game_id: felt252) -> bool {
        true
    }

    fn is_out_of_board(next_position: (u32, u32)) -> bool {
        let (n_x, n_y) = next_position;
        if n_x > 7 || n_x < 0 {
            return false;
        }
        if n_y > 7 || n_y < 0 {
            return false;
        }
        true
    }

    fn is_right_piece_move(
        maybe_piece: Option<PieceType>, curr_position: (u32, u32), next_position: (u32, u32)
    ) -> bool {
        let (c_x, c_y) = curr_position;
        let (n_x, n_y) = next_position;
        match maybe_piece {
            Option::Some(piece) => {
                match piece {
                    PieceType::WhitePawn => {
                        true
                    },
                    PieceType::WhiteKnight => {
                        if n_x == c_x + 2 && n_y == c_x + 1 {
                            return true;
                        }

                        panic(array!['Knight ilegal move'])
                    },
                    PieceType::WhiteBishop => {
                        true
                    },
                    PieceType::WhiteRook => {
                        true
                    },
                    PieceType::WhiteQueen => {
                        true
                    },
                    PieceType::WhiteKing => {
                        true
                    },
                    PieceType::BlackPawn => {
                        true
                    },
                    PieceType::BlackKnight => {
                        true
                    },
                    PieceType::BlackBishop => {
                        true
                    },
                    PieceType::BlackRook => {
                        true
                    },
                    PieceType::BlackQueen => {
                        true
                    },
                    PieceType::BlackKing => {
                        true
                    },
                }
            },
            Option::None(_) => panic(array!['Should not move empty square']),
        }
    }
}

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
    use dojo::world::IWorldDispatcher;
    use core::array::SpanTrait;

    #[test]
    #[available_gas(3000000000000000)]
    fn init_world_test() -> IWorldDispatcher {
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

        let mut calldata = array::ArrayTrait::<core::felt252>::new();
        calldata.append(white.into());
        calldata.append(black.into());
        world.execute('initiate_system'.into(), calldata);
        world
    }

    #[test]
    #[should_panic]
    fn test_ilegal_move() {
        let white = starknet::contract_address_const::<0x01>();
        let black = starknet::contract_address_const::<0x02>();
        let world = init_world_test();
        let game_id =  pedersen::pedersen(white.into(), black.into());

        let b1 = get!(world, (game_id, 1, 0), (Square));
        match b1.piece {
            Option::Some(piece) => {
                assert(piece == PieceType::WhiteKnight, 'should be White Knight');
            },
            Option::None(_) => assert(false, 'should have piece'),
        };

        // Knight cannot move to that square
        let mut move_calldata = array::ArrayTrait::<core::felt252>::new();
        move_calldata.append(1);
        move_calldata.append(0);
        move_calldata.append(2);
        move_calldata.append(3);
        move_calldata.append(white.into());
        move_calldata.append(game_id);
        world.execute('move_system'.into(), move_calldata);
    }


    #[test]
    #[available_gas(3000000000000000)]
    fn test_move() {
        let white = starknet::contract_address_const::<0x01>();
        let black = starknet::contract_address_const::<0x02>();
        let world = init_world_test();
        let game_id = pedersen::pedersen(white.into(), black.into());

        let a2 = get!(world, (game_id, 0, 1), (Square));
        match a2.piece {
            Option::Some(piece) => {
                assert(piece == PieceType::WhitePawn, 'should be White Pawn');
            },
            Option::None(_) => assert(false, 'should have piece'),
        };

        let mut move_calldata = array::ArrayTrait::<core::felt252>::new();
        move_calldata.append(0);
        move_calldata.append(1);
        move_calldata.append(0);
        move_calldata.append(2);
        move_calldata.append(white.into());
        move_calldata.append(game_id);
        world.execute('move_system'.into(), move_calldata);

        let c3 = get!(world, (game_id, 0, 2), (Square));
        match c3.piece {
            Option::Some(piece) => {
                assert(piece == PieceType::WhitePawn, 'should be White Knight');
            },
            Option::None(_) => assert(false, 'should have piece'),
        };
    }
}
