use array::ArrayTrait;

#[component]
struct Moves {
    remaining: u8, 
}

#[component]
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
        if self.x - self.y == 0_u32 {
            return bool::True(());
        }
        bool::False(())
    }

    fn is_equal(self: Position, b: Position) -> bool {
        self.x == b.x & self.y == b.y
    }
}

#[test]
#[available_gas(100000)]
fn test_position_is_zero() {
    assert(PositionTrait::is_zero(Position { x: 0_u32, y: 0_u32 }), 'not zero');
}

#[test]
#[available_gas(100000)]
fn test_position_is_equal() {
    assert(
        PositionTrait::is_equal(Position { x: 420_u32, y: 0_u32 }, Position { x: 420_u32, y: 0_u32 }), 'not equal'
    );
}
