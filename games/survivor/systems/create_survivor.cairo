#[system]
fn create_survivor(name: felt) {
    let survivor = world.spawn((
        Name::new(name),
        Health::new(100),
    ));
}
