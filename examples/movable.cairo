#[contract]
mod PositionComponent {
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

extern type Query<T>;

fn move(world: felt, query: Query::<PositionComponent::Position>) {
    // let query
    return ();
}
