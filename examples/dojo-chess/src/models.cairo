use array::ArrayTrait;
use debug::PrintTrait;
use starknet::ContractAddress;
use dojo::database::schema::{SchemaIntrospection, Ty, Enum, serialize_member_type};


#[derive(Model, Drop, Serde)]
struct Square {
    #[key]
    game_id: felt252,
    #[key]
    x: u32,
    #[key]
    y: u32,
    piece: PieceType,
}

#[derive(Serde, Drop, Copy, PartialEq, Introspect)]
enum PieceType {
    WhitePawn: (),
    WhiteKnight: (),
    WhiteBishop: (),
    WhiteRook: (),
    WhiteQueen: (),
    WhiteKing: (),
    BlackPawn: (),
    BlackKnight: (),
    BlackBishop: (),
    BlackRook: (),
    BlackQueen: (),
    BlackKing: (),
    None: (),
}

#[derive(Serde, Drop, Copy, PartialEq, Introspect)]
enum Color {
    White: (),
    Black: (),
    None: (),
}

#[derive(Model, Drop, Serde)]
struct Game {
    /// game id, computed as follows pedersen_hash(player1_address, player2_address)
    #[key]
    game_id: felt252,
    winner: Color,
    white: ContractAddress,
    black: ContractAddress
}

#[derive(Model, Drop, Serde)]
struct GameTurn {
    #[key]
    game_id: felt252,
    turn: Color
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
            Color::None(_) => {
                'None'.print();
            },
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
            PieceType::None(_) => {
                'None'.print();
            },
        }
    }
}

