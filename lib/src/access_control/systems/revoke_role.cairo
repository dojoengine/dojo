#[system]
mod RevokeRoleSystem {
    use dojo::access_control::components::role::Role;

    fn execute(target_id: felt252) {
        commands::<Role>::set(target_id.into(), Role { id: 0 });
    }
}
