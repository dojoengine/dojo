use array::ArrayTrait;
use array::SpanTrait;
use serde::Serde;

#[derive(Drop)]
struct Route {
    target_id: felt252,
    role_id: felt252,
    resource_id: felt252,
}

// TODO: replace with #[derive(Serde)] when supported
impl RouteSerde of Serde::<Route> {
    fn serialize(ref serialized: Array<felt252>, input: Route) {
        serialized.append(input.target_id);
        serialized.append(input.role_id);
        serialized.append(input.resource_id);
    }
    fn deserialize(ref serialized: Span<felt252>) -> Option<Route> {
        Option::Some(
            Route {
                target_id: *serialized.pop_front()?,
                role_id: *serialized.pop_front()?,
                resource_id: *serialized.pop_front()?,
            }
        )
    }
}
