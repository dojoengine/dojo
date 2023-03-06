use array::ArrayTrait;

#[component]
mod PositionComponent {
    #[derive(Copy, Drop)]
    struct Position {
        x: felt,
        y: felt
    }

    #[view]
    fn is_zero(self: Position) -> bool {
        match self.x - self.y {
            0 => bool::True(()),
            _ => bool::False(()),
        }
    }

    #[view]
    fn is_equal(self: Position, b: Position) -> bool {
        self.x == b.x & self.y == b.y
    }
}

#[test]
#[available_gas(100000)]
fn test_position_is_zero() {
    assert(PositionComponent::is_zero(0), 'not zero');
}

#[test]
#[available_gas(100000)]
fn test_position_is_equal() {
    assert(PositionComponent::is_equal(0, PositionComponent::Position { x: 0, y: 0 }), 'not equal');
}
