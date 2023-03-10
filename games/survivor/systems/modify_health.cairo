#[system]
fn adjust_health(query: Query<(Health)>, value: felt) {
    let mut current_value = Health::get();
    Health.set(current_value + value);
}
