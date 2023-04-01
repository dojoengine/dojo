#[system]
mod RevokeRole {
    use dojo::auth::components::role::Role;

    fn execute(target_id: felt252) {
        commands::<Role>::set(target_id.into(), Role { id: 0 });
    }
}
