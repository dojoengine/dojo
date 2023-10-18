use dojo_chess::models::PieceType;
use starknet::ContractAddress;

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
    maybe_piece: PieceType, curr_position: (u32, u32), next_position: (u32, u32)
) -> bool {
    let (c_x, c_y) = curr_position;
    let (n_x, n_y) = next_position;
    match maybe_piece {
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
        PieceType::None(_) => panic(array!['Should not move empty square']),
    }
}

