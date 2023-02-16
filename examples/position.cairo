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
#[available_gas(20000)]
fn test_position_is_zero() {
    let mut retdata = PositionComponent::__external::get_plus_2(single_element_arr(1));
    pop_and_compare(ref retdata, 3, 'Wrong result');
    assert_empty(retdata);
}
