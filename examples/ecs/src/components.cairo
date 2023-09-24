use array::ArrayTrait;
use core::debug::PrintTrait;
use starknet::ContractAddress;
use dojo::database::schema::{
    EnumMember, Member, Ty, Struct, SchemaIntrospection, serialize_member, serialize_member_type
};

#[derive(Serde, Copy, Drop)]
enum Direction {
    None: (),
    Left: (),
    Right: (),
    Up: (),
    Down: (),
}

impl DirectionSchemaIntrospectionImpl of SchemaIntrospection<Direction> {
    #[inline(always)]
    fn size() -> usize {
        1
    }

    #[inline(always)]
    fn layout(ref layout: Array<u8>) {
        layout.append(8);
    }

    #[inline(always)]
    fn ty() -> Ty {
        Ty::Enum(
            EnumMember {
                name: 'Direction',
                attrs: array![].span(),
                values: array![
                    serialize_member_type(@Ty::Simple('None')),
                    serialize_member_type(@Ty::Simple('Left')),
                    serialize_member_type(@Ty::Simple('Right')),
                    serialize_member_type(@Ty::Simple('Up')),
                    serialize_member_type(@Ty::Simple('Down'))
                ]
                    .span()
            }
        )
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

#[derive(Copy, Drop, Serde, Print, Schema)]
struct Vec2 {
    x: u32,
    y: u32
}

#[derive(Component, Copy, Drop, Print, Serde)]
struct Position {
    #[key]
    player: ContractAddress,
    vec: Vec2,
}

trait Vec2Trait {
    fn is_zero(self: Vec2) -> bool;
    fn is_equal(self: Vec2, b: Vec2) -> bool;
}

impl Vec2Impl of Vec2Trait {
    fn is_zero(self: Vec2) -> bool {
        if self.x - self.y == 0 {
            return true;
        }
        false
    }

    fn is_equal(self: Vec2, b: Vec2) -> bool {
        self.x == b.x && self.y == b.y
    }
}

#[cfg(test)]
mod tests {
    use debug::PrintTrait;
    use super::{Position, Vec2, Vec2Trait};

    #[test]
    #[available_gas(100000)]
    fn test_vec_is_zero() {
        assert(Vec2Trait::is_zero(Vec2 { x: 0, y: 0 }), 'not zero');
    }

    #[test]
    #[available_gas(100000)]
    fn test_vec_is_equal() {
        let position = Vec2 { x: 420, y: 0 };
        assert(position.is_equal(Vec2 { x: 420, y: 0 }), 'not equal');
    }
}
