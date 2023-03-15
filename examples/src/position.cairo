use array::ArrayTrait;

#[derive(Component)]
struct Health {
    name: felt252,
}

#[derive(Component)]
struct Position {
    x: felt252,
    y: felt252
}

trait PositionTrait {
    fn is_zero(self: Position) -> bool;
    fn is_equal(self: Position, b: Position) -> bool;
}

impl PositionImpl of PositionTrait {
    fn is_zero(self: Position) -> bool {
        match self.x - self.y {
            0 => bool::True(()),
            _ => bool::False(()),
        }
    }

    fn is_equal(self: Position, b: Position) -> bool {
        self.x == b.x & self.y == b.y
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
