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

fn single_element_arr(value: felt) -> Array::<felt> {
    let mut arr = ArrayTrait::new();
    arr.append(value);
    arr
}

fn pop_and_compare(ref arr: Array::<felt>, value: felt, err: felt) {
    match arr.pop_front() {
        Option::Some(x) => {
            assert(x == value, err);
        },
        Option::None(_) => {
            panic(single_element_arr('Got empty result data'))
        },
    };
}

#[test]
#[available_gas(100000)]
fn test_position_is_zero() {
    let mut is_zero = PositionComponent::__external::is_zero(single_element_arr(1));
    pop_and_compare(ref is_zero, 1, 'not zero');
}
