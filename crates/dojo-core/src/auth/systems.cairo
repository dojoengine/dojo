use dojo_core::integer::u250;

#[derive(Drop, Serde)]
struct Route {
    target_id: u250,
    role_id: u250,
    resource_id: u250,
}

#[system]
mod RouteAuth {
    use traits::Into;

    use dojo_core::auth::components::Status;
    use dojo_core::auth::components::Role;
    use super::Route;

    use starknet::ContractAddress;

    fn execute(route: Route) {
        // TODO: Figure out how to store multiple roles per entity
        // Set role
        commands::set_entity(route.target_id.into(), (Role { id: route.role_id }));

        // Set status
        commands::set_entity((route.role_id, route.resource_id).into(), (Status { is_authorized: bool::True(()) }));
    }
}

#[system]
mod Authorize {
    use traits::Into;
    use dojo_core::integer::u250;
    use dojo_core::auth::components::Status;
    use dojo_core::auth::components::Role;

    fn execute(caller_id: u250, resource_id: u250) {
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
    use dojo_core::integer::u250;
    use dojo_core::auth::components::Role;

    fn execute(target_id: u250, role_id: u250) {
        commands::set_entity(target_id.into(), (Role { id: role_id }));
    }
}

#[system]
mod GrantResource {
    use traits::Into;
    use dojo_core::integer::u250;
    use dojo_core::auth::components::Status;

    fn execute(role_id: u250, resource_id: u250) {
        commands::set_entity((role_id, resource_id).into(), (bool::True(())));
    }
}

#[system]
mod RevokeRole {
    use traits::Into;
    use array::ArrayTrait;
    use dojo_core::integer::u250;
    use dojo_core::auth::components::Role;

    fn execute(target_id: u250) {
        commands::set_entity(target_id.into(), (Role { id: 0.into() }));
    }
}

#[system]
mod RevokeResource {
    use traits::Into;
    use dojo_core::integer::u250;
    use dojo_core::auth::components::Status;

    fn execute(role_id: u250, resource_id: u250) {
        commands::set_entity((role_id, resource_id).into(), (bool::False(())));
    }
}
