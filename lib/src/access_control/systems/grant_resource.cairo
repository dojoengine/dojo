#[system]
mod GrantResourceSystem {
    use dojo::access_control::components::authorization_status::AuthorizationStatus;

    fn execute(role_id: felt252, resource_id: felt252) {
        commands::<AuthorizationStatus>::set((role_id, resource_id).into(), bool::True(()));
    }
}
