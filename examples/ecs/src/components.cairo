use array::{ArrayTrait, SpanTrait};
use dojo::packable::{Packable, PackableU8, PackableU32};
use option::OptionTrait;

#[derive(Component, Copy, Drop, Serde)]
struct Moves {
    remaining: u8, 
}

#[derive(Component, Copy, Drop, Serde)]
struct Position {
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

impl PackableMoves of Packable<Moves> {
    #[inline(always)]
    fn pack(self: @Moves, ref packing: felt252, ref packing_offset: u8, ref packed: Array<felt252>) {
        self.remaining.pack(ref packing, ref packing_offset, ref packed)
    }
    #[inline(always)]
    fn unpack(ref packed: Span<felt252>, ref unpacking: felt252, ref unpacking_offset: u8) -> Option<Moves> {
        Option::Some(Moves { remaining: Packable::<u8>::unpack(ref packed, ref unpacking, ref unpacking_offset).unwrap()})
    }
    #[inline(always)]
    fn size() -> usize {
        Packable::<u8>::size()
    }
}

impl PackablePosition of Packable<Position> {
    #[inline(always)]
    fn pack(self: @Position, ref packing: felt252, ref packing_offset: u8, ref packed: Array<felt252>) {
        self.x.pack(ref packing, ref packing_offset, ref packed);
        self.y.pack(ref packing, ref packing_offset, ref packed)
    }
    #[inline(always)]
    fn unpack(ref packed: Span<felt252>, ref unpacking: felt252, ref unpacking_offset: u8) -> Option<Position> {
        Option::Some(Position { 
            x: Packable::<u32>::unpack(ref packed, ref unpacking, ref unpacking_offset).unwrap(),
            y: Packable::<u32>::unpack(ref packed, ref unpacking, ref unpacking_offset).unwrap()
        })
    }
    #[inline(always)]
    fn size() -> usize {
        Packable::<u32>::size() + Packable::<u32>::size()
    }
}

#[test]
#[available_gas(100000)]
fn test_position_is_zero() {
    assert(PositionTrait::is_zero(Position { x: 0, y: 0 }), 'not zero');
}

#[test]
#[available_gas(100000)]
fn test_position_is_equal() {
    assert(
        PositionTrait::is_equal(Position { x: 420, y: 0 }, Position { x: 420, y: 0 }), 'not equal'
    );
}
