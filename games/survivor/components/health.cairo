use array::ArrayTrait;

#[derive(Component)]
struct Health {
    value: felt,
}

impl Health of Component {
    #[view]
    fn is_zero(self: Health) -> bool {
        match self.value {
            0 => bool::True(()),
            _ => bool::False(()),
        }
    }
}

#[test]
#[available_gas(100000)]
fn test_health_is_zero() {
    assert(HealthComponent::is_zero(0), 'not zero');
}

// #[test]
// #[available_gas(100000)]
// fn test_position_is_equal() {
//     assert(HealthComponent::is_equal(0, HealthComponent::Health { value: 0 }), 'not equal');
// }
