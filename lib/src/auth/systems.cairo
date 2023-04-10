#[system]
mod AuthRouting {
    use array::SpanTrait;
    use option::OptionTrait;

    use dojo::auth::components::AuthorizationStatus;
    use dojo::auth::components::Role;

    fn execute(
        ref target_ids: Span<felt252>, ref roles: Span<felt252>, ref resources: Span<felt252>
    ) {
        let target_ids_len = target_ids.len();
        let roles_len = roles.len();
        let resources_len = resources.len();

        assert((target_ids_len == roles_len) & (roles_len == resources_len), 'length mismatch');

        _set_authorization_routing(ref target_ids, ref roles, ref resources);
    }

    fn _set_authorization_routing(
        ref target_ids: Span<felt252>, ref roles: Span<felt252>, ref resources: Span<felt252>
    ) {
        if target_ids.is_empty() {
            return ();
        }

        let target_id = *target_ids.pop_front().unwrap();
        let role_id = *roles.pop_front().unwrap();
        let resource_id = *resources.pop_front().unwrap();

        commands::<Role>::set(target_id.into(), Role { id: role_id });
        commands::<AuthorizationStatus>::set((role_id, resourc_id).into(), bool::True(()));

        _set_authorization_routing(ref target_ids, ref roles, ref resources);
    }
}

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
