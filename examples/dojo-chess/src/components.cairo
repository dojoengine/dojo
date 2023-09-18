use debug::PrintTrait;
use starknet::ContractAddress;

#[derive(Component, Drop, SerdeLen, Serde)]
struct Square {
    #[key]
    game_id: felt252,
    #[key]
    x: u32,
    #[key]
    y: u32,
    piece: Option<PieceType>,
}

#[derive(Serde, Drop, Copy, PartialEq)]
enum PieceType {
    WhitePawn,
    WhiteKnight,
    WhiteBishop,
    WhiteRook,
    WhiteQueen,
    WhiteKing,
    BlackPawn,
    BlackKnight,
    BlackBishop,
    BlackRook,
    BlackQueen,
    BlackKing,
}


#[derive(Serde, Drop, Copy, PartialEq)]
enum Color {
    White,
    Black,
}

#[derive(Component, Drop, SerdeLen, Serde)]
struct Game {
    /// game id, computed as follows pedersen_hash(player1_address, player2_address)
    #[key]
    game_id: felt252,
    winner: Option<Color>,
    white: ContractAddress,
    black: ContractAddress
}

#[derive(Component, Drop, SerdeLen, Serde)]
struct GameTurn {
    #[key]
    game_id: felt252,
    turn: Color
}


//Assigning storage types for enum
impl GameTurnOptionColorStorageSize of dojo::StorageSize<Color> {
    #[inline(always)]
    fn unpacked_size() -> usize {
        1
    }

    #[inline(always)]
    fn packed_size() -> usize {
        256
    }
}

impl GameOptionColorStorageSize of dojo::StorageSize<Option<Color>> {
    #[inline(always)]
    fn unpacked_size() -> usize {
        1
    }

    #[inline(always)]
    fn packed_size() -> usize {
        256
    }
}

impl PieceOptionStoragSize of dojo::StorageSize<Option<PieceType>> {
    #[inline(always)]
    fn unpacked_size() -> usize {
        2
    }

    #[inline(always)]
    fn packed_size() -> usize {
        256
    }
}

//printing trait for debug

impl ColorPrintTrait of PrintTrait<Color> {
    #[inline(always)]
    fn print(self: Color) {
        match self {
            Color::White(_) => {
                'White'.print();
            },
            Color::Black(_) => {
                'Black'.print();
            },
        }
    }
}


impl ColorOptionPrintTrait of PrintTrait<Option<Color>> {
    #[inline(always)]
        fn print(self: Option<Color>) {
        match self {
            Option::Some(color_type) => {
                color_type.print();
            },
            Option::None(_) => {
                'None'.print();
            }
        }
    }
}

impl BoardPrintTrait of PrintTrait<(u32, u32)> {
    #[inline(always)]
    fn print(self: (u32, u32)) {
        let (x, y): (u32, u32) = self;
        x.print();
        y.print();
    }
}


impl PieceTypeOptionPrintTrait of PrintTrait<Option<PieceType>> {
    #[inline(always)]
    fn print(self: Option<PieceType>) {
        match self {
            Option::Some(piece_type) => {
                piece_type.print();
            },
            Option::None(_) => {
                'None'.print();
            }
        }
    }
}


impl PieceTypePrintTrait of PrintTrait<PieceType> {
    #[inline(always)]
    fn print(self: PieceType) {
        match self {
            PieceType::WhitePawn(_) => {
                'WhitePawn'.print();
            },
            PieceType::WhiteKnight(_) => {
                'WhiteKnight'.print();
            },
            PieceType::WhiteBishop(_) => {
                'WhiteBishop'.print();
            },
            PieceType::WhiteRook(_) => {
                'WhiteRook'.print();
            },
            PieceType::WhiteQueen(_) => {
                'WhiteQueen'.print();
            },
            PieceType::WhiteKing(_) => {
                'WhiteKing'.print();
            },
            PieceType::BlackPawn(_) => {
                'BlackPawn'.print();
            },
            PieceType::BlackKnight(_) => {
                'BlackKnight'.print();
            },
            PieceType::BlackBishop(_) => {
                'BlackBishop'.print();
            },
            PieceType::BlackRook(_) => {
                'BlackRook'.print();
            },
            PieceType::BlackQueen(_) => {
                'BlackQueen'.print();
            },
            PieceType::BlackKing(_) => {
                'BlackKing'.print();
            },
        }
    }
}

