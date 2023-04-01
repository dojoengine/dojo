#[system]
mod Authorize {
    use dojo::auth::components::authorization_status::AuthorizationStatus;
    use dojo::auth::components::role::Role;

    fn execute(caller_id: felt252, resource_id: felt252) {
        let role = commands::<Role>::get(caller_id.into());
        let authorization_status = commands::<AuthorizationStatus>::get(
            (role.role, resource_id).into()
        );
        assert(authorization_status.is_authorized, 'not authorized');
    }
}
