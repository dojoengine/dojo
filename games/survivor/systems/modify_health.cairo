#[system]
fn adjust_health(query: Query<(Health)>, value: felt) {
    let current_value = Health::get();
    Health.set(current_value + value);
}
