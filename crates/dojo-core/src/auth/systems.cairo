use dojo_core::integer::u250;

#[derive(Drop, Serde)]
struct Route {
    target_id: u250,
    role_id: u250,
    resource_id: u250,
}

trait RouteTrait {
    fn new(target_id: u250, role_id: u250, resource_id: u250) -> Route;
}

impl RouteImpl of RouteTrait {
    fn new(target_id: u250, role_id: u250, resource_id: u250) -> Route {
        Route {
            target_id,
            role_id,
            resource_id,
        }
    }
}

#[system]
mod RouteAuth {
    use traits::Into;

    use dojo_core::auth::components::Status;
    use dojo_core::auth::components::Role;
    use super::Route;

    use starknet::ContractAddress;

    fn execute(route: Route) {
        // Set scoped role
        commands::set_entity((route.target_id, route.resource_id).into(), (Role { id: route.role_id }));

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

    fn execute(caller_id: u250, resource_id: u250) -> bool {
        // Get World level role
        let maybe_role = commands::<Role>::try_entity(caller_id.into());
        let role = match maybe_role {
            Option::Some(role) => role.id.into(),
            Option::None(()) => 0,
        };

        // Get component-scoped role
        let maybe_scoped_role = commands::<Role>::try_entity((caller_id, resource_id).into());
        let scoped_role = match maybe_scoped_role {
            Option::Some(scoped_role) => scoped_role.id.into(),
            Option::None(_) => 0,
        }; 

        // Get authorization status for scoped role
        let maybe_authorization_status = commands::<Status>::try_entity(
            (scoped_role, resource_id).into()
        );
        let authorization_status = match maybe_authorization_status {
            Option::Some(authorization_status) => authorization_status.is_authorized,
            Option::None(_) => bool::False(()),
        };
        // Authorize if role is Admin or authorization status is true
        role == 'Admin' | authorization_status
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
mod GrantScopedRole {
    use traits::Into;
    use array::ArrayTrait;
    use dojo_core::integer::u250;
    use dojo_core::auth::components::Role;

    fn execute(target_id: u250, role_id: u250, resource_id: u250) {
        commands::set_entity((target_id, resource_id).into(), (Role { id: role_id }));
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
mod RevokeScopedRole {
    use traits::Into;
    use array::ArrayTrait;
    use dojo_core::integer::u250;
    use dojo_core::auth::components::Role;

    fn execute(target_id: u250, resource_id: u250) {
        commands::set_entity((target_id, resource_id).into(), (Role { id: 0.into() }));
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
