#[system]
mod Authorize {
    use dojo::auth::components::AuthorizationStatus;
    use dojo::auth::components::Role;

    fn execute(caller_id: felt252, resource_id: felt252) {
        let role = commands::<Role>::get(caller_id.into());
        let authorization_status = commands::<AuthorizationStatus>::get(
            (role.role, resource_id).into()
        );
        assert(authorization_status.is_authorized, 'not authorized');
    }
}

#[system]
mod GrantRole {
    use dojo::auth::components::Role;

    fn execute(target_id: felt252, role_id: felt252) {
        commands::<Role>::set(target_id.into(), Role { id: role_id });
    }
}

#[system]
mod GrantResource {
    use dojo::auth::components::AuthorizationStatus;

    fn execute(role_id: felt252, resource_id: felt252) {
        commands::<AuthorizationStatus>::set((role_id, resource_id).into(), bool::True(()));
    }
}

#[system]
mod RevokeRole {
    use dojo::auth::components::Role;

    fn execute(target_id: felt252) {
        commands::<Role>::set(target_id.into(), Role { id: 0 });
    }
}

#[system]
mod RevokeResource {
    use dojo::auth::components::AuthorizationStatus;

    fn execute(role_id: felt252, resource_id: felt252) {
        commands::<AuthorizationStatus>::set((role_id, resource_id).into(), bool::False(()));
    }
}
