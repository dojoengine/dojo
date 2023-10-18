use starknet::ContractAddress;
use dojo::world::{IWorldDispatcher, IWorldDispatcherTrait};
#[starknet::interface]
trait IActions<ContractState> {
    fn move(
        self: @ContractState,
        curr_position: (u32, u32),
        next_position: (u32, u32),
        caller: ContractAddress, //player
        game_id: felt252
    );
    fn spawn_game(
        self: @ContractState, white_address: ContractAddress, black_address: ContractAddress,
    );
}
#[starknet::contract]
mod actions {
    use dojo::world::{IWorldDispatcher, IWorldDispatcherTrait};
    use debug::PrintTrait;
    use starknet::ContractAddress;
    use dojo_chess::models::{Color, Square, PieceType, Game, GameTurn};
    use super::IActions;
    use dojo_chess::utils::{is_out_of_board, is_right_piece_move, is_piece_is_mine};

    #[storage]
    struct Storage {
        world_dispatcher: IWorldDispatcher,
    }

    #[external(v0)]
    impl PlayerActionsImpl of IActions<ContractState> {
        fn spawn_game(
            self: @ContractState, white_address: ContractAddress, black_address: ContractAddress
        ) {
            let world = self.world_dispatcher.read();
            let game_id = pedersen::pedersen(white_address.into(), black_address.into());
            set!(
                world,
                (
                    Game {
                        game_id: game_id,
                        winner: Color::None(()),
                        white: white_address,
                        black: black_address,
                    },
                    GameTurn { game_id: game_id, turn: Color::White(()), },
                )
            );

            set!(world, (Square { game_id: game_id, x: 0, y: 0, piece: PieceType::WhiteRook }));

            set!(world, (Square { game_id: game_id, x: 0, y: 1, piece: PieceType::WhitePawn }));

            set!(world, (Square { game_id: game_id, x: 1, y: 6, piece: PieceType::BlackPawn }));

            set!(world, (Square { game_id: game_id, x: 1, y: 0, piece: PieceType::WhiteKnight }));
        }

        fn move(
            self: @ContractState,
            curr_position: (u32, u32),
            next_position: (u32, u32),
            caller: ContractAddress, //player
            game_id: felt252
        ) {
            let world = self.world_dispatcher.read();

            let (current_x, current_y) = curr_position;
            let (next_x, next_y) = next_position;
            current_x.print();
            current_y.print();

            next_x.print();
            next_y.print();

            let mut current_square = get!(world, (game_id, current_x, current_y), (Square));

            // check if next_position is out of board or not
            assert(is_out_of_board(next_position), 'Should be inside board');

            // check if this is the right piece type move
            assert(
                is_right_piece_move(current_square.piece, curr_position, next_position),
                'Should be right piece move'
            );
            let target_piece = current_square.piece;
            // make current_square piece none and move piece to next_square 
            current_square.piece = PieceType::None(());
            let mut next_square = get!(world, (game_id, next_x, next_y), (Square));

            // check the piece already in next_suqare
            let maybe_next_square_piece = next_square.piece;

            if maybe_next_square_piece == PieceType::None(()) {
                next_square.piece = target_piece;
            } else {
                if is_piece_is_mine(maybe_next_square_piece) {
                    panic(array!['Already same color piece exist'])
                } else {
                    next_square.piece = target_piece;
                }
            }

            set!(world, (next_square));
            set!(world, (current_square));
        }
    }
}

#[cfg(test)]
mod tests {
    use starknet::ContractAddress;
    use dojo::test_utils::{spawn_test_world, deploy_contract};
    use dojo_chess::models::{Game, game, GameTurn, game_turn, Square, square, PieceType};

    use dojo_chess::actions_contract::actions;
    use starknet::class_hash::Felt252TryIntoClassHash;
    use dojo::world::IWorldDispatcherTrait;
    use dojo::world::IWorldDispatcher;
    use core::array::SpanTrait;
    use super::{IActionsDispatcher, IActionsDispatcherTrait};

    // helper setup function
    // reusable function for tests
    fn setup_world() -> (IWorldDispatcher, IActionsDispatcher) {
        // models
        let mut models = array![
            game::TEST_CLASS_HASH, game_turn::TEST_CLASS_HASH, square::TEST_CLASS_HASH
        ];
        // deploy world with models
        let world = spawn_test_world(models);

        // deploy systems contract
        let contract_address = world
            .deploy_contract('salt', actions::TEST_CLASS_HASH.try_into().unwrap());
        let actions_system = IActionsDispatcher { contract_address };

        (world, actions_system)
    }

    #[test]
    #[available_gas(3000000000000000)]
    fn test_initiate() {
        let white = starknet::contract_address_const::<0x01>();
        let black = starknet::contract_address_const::<0x02>();

        let (world, actions_system) = setup_world();

        //system calls
        actions_system.spawn_game(white, black);
        let game_id = pedersen::pedersen(white.into(), black.into());

        //get game
        let game = get!(world, game_id, (Game));
        assert(game.white == white, 'white address is incorrect');
        assert(game.black == black, 'black address is incorrect');

        //get a1 square
        let a1 = get!(world, (game_id, 0, 0), (Square));
        assert(a1.piece == PieceType::WhiteRook, 'should be White Rook');
        assert(a1.piece != PieceType::None, 'should have piece');
    }


    #[test]
    #[available_gas(3000000000000000)]
    fn test_move() {
        let white = starknet::contract_address_const::<0x01>();
        let black = starknet::contract_address_const::<0x02>();

        let (world, actions_system) = setup_world();
        actions_system.spawn_game(white, black);

        let game_id = pedersen::pedersen(white.into(), black.into());

        let a2 = get!(world, (game_id, 0, 1), (Square));
        assert(a2.piece == PieceType::WhitePawn, 'should be White Pawn');
        assert(a2.piece != PieceType::None, 'should have piece');

        actions_system.move((0, 1), (0, 2), white.into(), game_id);

        let c3 = get!(world, (game_id, 0, 2), (Square));
        assert(c3.piece == PieceType::WhitePawn, 'should be White Pawn');
        assert(c3.piece != PieceType::None, 'should have piece');
    }
    #[test]
    #[should_panic]
    fn test_ilegal_move() {
        let white = starknet::contract_address_const::<0x01>();
        let black = starknet::contract_address_const::<0x02>();
        let (world, actions_system) = setup_world();
        let game_id = pedersen::pedersen(white.into(), black.into());

        let b1 = get!(world, (game_id, 1, 0), (Square));
        assert(b1.piece == PieceType::WhiteKnight, 'should be White Knight');

        // Knight cannot move to that square
        actions_system.move((1, 0), (2, 3), white.into(), game_id);
    }
}
