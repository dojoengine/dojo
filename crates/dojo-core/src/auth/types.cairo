#[derive(Drop, Serde)]
struct Route {
    target_id: felt252,
    role_id: felt252,
    resource_id: felt252,
}
