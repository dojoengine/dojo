#[derive(Component)]
struct Name {
    value: felt,
}

impl Name of Component {
    #[view]
    fn is_zero(self: Name) -> bool {
        match self.value {
            0 => bool::True(()),
            _ => bool::False(()),
        }
    }
}

#[test]
#[available_gas(100000)]
fn test_name_is_zero() {
    assert(NameComponent::is_zero(0), 'not zero');
}

// #[test]
// #[available_gas(100000)]
// fn test_position_is_equal() {
//     assert(HealthComponent::is_equal(0, HealthComponent::Health { value: 0 }), 'not equal');
// }
