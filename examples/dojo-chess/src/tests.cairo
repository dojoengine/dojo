#[cfg(test)]
mod tests {
    use starknet::ContractAddress;
    use dojo::test_utils::spawn_test_world;
    use dojo_chess::models::{Game, game, GameTurn, game_turn, Square, square, PieceType};

    use dojo_chess::actions_contract::actions;
    use array::ArrayTrait;
    use core::traits::Into;
    use dojo::world::IWorldDispatcherTrait;
    use core::array::SpanTrait;
    use dojo_chess::actions_contract::tests::setup_world;
    use dojo_chess::actions_contract::{IActionsDispatcher, IActionsDispatcherTrait};


    #[test]
    #[available_gas(3000000000000000)]
    fn integration() {
        let white = starknet::contract_address_const::<0x01>();
        let black = starknet::contract_address_const::<0x02>();

        let (world, actions_system) = setup_world();

        //system calls
        actions_system.spawn_game(white, black);
        let game_id = pedersen::pedersen(white.into(), black.into());

        //White pawn is now in (0,1)
        let a2 = get!(world, (game_id, 0, 1), (Square));
        assert(a2.piece == PieceType::WhitePawn, 'should be White Pawn in (0,1)');
        assert(a2.piece != PieceType::None, 'should have piece in (0,1)');

        //Black pawn is now in (1,6)
        let b7 = get!(world, (game_id, 1, 6), (Square));
        assert(b7.piece == PieceType::BlackPawn, 'should be Black Pawn in (1,6)');
        assert(b7.piece != PieceType::None, 'should have piece in (1,6)');

        //Move White Pawn to (0,3)
        actions_system.move((0, 1), (0, 3), white.into(), game_id);

        //White pawn is now in (0,3)
        let a4 = get!(world, (game_id, 0, 3), (Square));
        assert(a4.piece == PieceType::WhitePawn, 'should be White Pawn in (0,3)');
        assert(a4.piece != PieceType::None, 'should have piece in (0,3)');

        //Move black Pawn to (1,4)
        actions_system.move((1, 6), (1, 4), white.into(), game_id);

        //Black pawn is now in (1,4)
        let b5 = get!(world, (game_id, 1, 4), (Square));
        assert(b5.piece == PieceType::BlackPawn, 'should be Black Pawn in (1,4)');
        assert(b5.piece != PieceType::None, 'should have piece in (1,4)');

        // Move White Pawn to (1,4)
        // Capture black pawn
        actions_system.move((0, 3), (1, 4), white.into(), game_id);

        let b5 = get!(world, (game_id, 1, 4), (Square));
        assert(b5.piece == PieceType::WhitePawn, 'should be White Pawn in (1,4)');
        assert(b5.piece != PieceType::None, 'should have piece in (1,4)');
    }
}
