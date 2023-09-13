use array::ArrayTrait;
use core::debug::PrintTrait;
use starknet::ContractAddress;

#[derive(Serde, Copy, Drop)]
enum Direction {
    None: (),
    Left: (),
    Right: (),
    Up: (),
    Down: (),
}

impl DirectionSchemaIntrospectionImpl of dojo::SchemaIntrospection<Direction> {
    #[inline(always)]
    fn size() -> usize {
        1
    }

    #[inline(always)]
    fn layout(ref layout: Array<u8>) {
        layout.append(8);
    }

    #[inline(always)]
    fn schema(ref schema: Array<dojo::Member>) {
        schema.append(dojo::Member {
            name: '',
            ty: 'Direction',
        });
    }
}

impl DirectionPrintImpl of PrintTrait<Direction> {
    fn print(self: Direction) {
        match self {
            Direction::None(()) => 0.print(),
            Direction::Left(()) => 1.print(),
            Direction::Right(()) => 2.print(),
            Direction::Up(()) => 3.print(),
            Direction::Down(()) => 4.print(),
        }
    }
}

impl DirectionIntoFelt252 of Into<Direction, felt252> {
    fn into(self: Direction) -> felt252 {
        match self {
            Direction::None(()) => 0,
            Direction::Left(()) => 1,
            Direction::Right(()) => 2,
            Direction::Up(()) => 3,
            Direction::Down(()) => 4,
        }
    }
}

#[derive(Component, Copy, Drop, Serde)]
struct Moves {
    #[key]
    player: ContractAddress,
    remaining: u8,
    last_direction: Direction
}

#[derive(Component, Copy, Drop, Serde)]
struct Position {
    #[key]
    player: ContractAddress,
    x: u32,
    y: u32
}

trait PositionTrait {
    fn is_zero(self: Position) -> bool;
    fn is_equal(self: Position, b: Position) -> bool;
}

impl PositionImpl of PositionTrait {
    fn is_zero(self: Position) -> bool {
        if self.x - self.y == 0 {
            return true;
        }
        false
    }

    fn is_equal(self: Position, b: Position) -> bool {
        self.x == b.x && self.y == b.y
    }
}

#[cfg(test)]
mod tests {
    use debug::PrintTrait;
    use super::{Position, PositionTrait};

    #[test]
    #[available_gas(100000)]
    fn test_position_is_zero() {
        let player = starknet::contract_address_const::<0x0>();
        assert(PositionTrait::is_zero(Position { player, x: 0, y: 0 }), 'not zero');
    }

    #[test]
    #[available_gas(100000)]
    fn test_position_is_equal() {
        let player = starknet::contract_address_const::<0x0>();
        let position = Position { player, x: 420, y: 0 };
        position.print();
        assert(PositionTrait::is_equal(position, Position { player, x: 420, y: 0 }), 'not equal');
    }
}
