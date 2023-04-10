#[system]
mod AuthRouting {
    use traits::Into;
    use array::ArrayTrait;

    use dojo_core::auth::components::Status;
    use dojo_core::auth::components::Role;

    fn execute(
        ref target_ids: Array<felt252>, ref roles: Array<felt252>, ref resources: Array<felt252>
    ) {
        let target_ids_len = target_ids.len();
        let roles_len = roles.len();
        let resources_len = resources.len();

        assert((target_ids_len == roles_len) & (roles_len == resources_len), 'length mismatch');

        _set_authorization_routing(ref target_ids, ref roles, ref resources);
    }

    fn _set_authorization_routing(
        ref target_ids: Array<felt252>, ref roles: Array<felt252>, ref resources: Array<felt252>
    ) {
        if target_ids.is_empty() {
            return ();
        }

        let target_id = target_ids.pop_front().unwrap();
        let role_id = roles.pop_front().unwrap();
        let resource_id = resources.pop_front().unwrap();

        // TODO: fails due to commands::set_entity not supported outside of execute
        // commands::set_entity(target_id.into(), (Role { id: role_id }));
        // commands::set_entity((role_id, resource_id).into(), (bool::True(())));

        _set_authorization_routing(ref target_ids, ref roles, ref resources);
    }
}

#[system]
mod Authorize {
    use traits::Into;

    use dojo_core::auth::components::Status;
    use dojo_core::auth::components::Role;

    fn execute(caller_id: felt252, resource_id: felt252) {
        let role = commands::<Role>::entity(caller_id.into());
        let authorization_status = commands::<Status>::entity(
            (role.id, resource_id).into()
        );
        assert(authorization_status.is_authorized, 'not authorized');
    }
}

#[system]
mod GrantRole {
    use traits::Into;
    use array::ArrayTrait;

    use dojo_core::auth::components::Role;

    fn execute(target_id: felt252, role_id: felt252) {
        commands::set_entity(target_id.into(), (Role { id: role_id }));
    }
}

#[system]
mod GrantResource {
    use traits::Into;

    use dojo_core::auth::components::Status;

    fn execute(role_id: felt252, resource_id: felt252) {
        commands::set_entity((role_id, resource_id).into(), (bool::True(())));
    }
}

#[system]
mod RevokeRole {
    use traits::Into;
    use array::ArrayTrait;

    use dojo_core::auth::components::Role;

    fn execute(target_id: felt252) {
        commands::set_entity(target_id.into(), (Role { id: 0 }));
    }
}

#[system]
mod RevokeResource {
    use traits::Into;

    use dojo_core::auth::components::Status;

    fn execute(role_id: felt252, resource_id: felt252) {
        commands::set_entity((role_id, resource_id).into(), (bool::False(())));
    }
}
