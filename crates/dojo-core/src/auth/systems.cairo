#[derive(Drop, Serde)]
struct Route {
    target_id: felt252,
    role_id: felt252,
    resource_id: felt252,
}

trait RouteTrait {
    fn new(target_id: felt252, role_id: felt252, resource_id: felt252) -> Route;
}

impl RouteImpl of RouteTrait {
    fn new(target_id: felt252, role_id: felt252, resource_id: felt252) -> Route {
        Route { target_id, role_id, resource_id,  }
    }
}

#[system]
mod RouteAuth {
    use traits::Into;

    use dojo_core::auth::components::{AuthStatus, AuthRole};
    use super::Route;

    use starknet::ContractAddress;

    fn execute(route: Route) {
        // Set scoped role
        commands::set_entity(
            (route.target_id, route.resource_id).into(), (AuthRole { id: route.role_id })
        );

        // Set status
        commands::set_entity(
            (route.role_id, route.resource_id).into(), (AuthStatus { is_authorized: true })
        );
    }
}

#[system]
mod IsAccountAdmin {
    use traits::Into;
    use box::BoxTrait;
    use dojo_core::auth::components::{AuthStatus, AuthRole};
    use dojo_core::world::World;

    fn execute(ctx: Context) -> bool {
        // Get calling account contract address
        let caller = ctx.caller_account;
        let role = commands::<AuthRole>::entity(caller.into());
        // Authorize if role is Admin
        role.id.into() == World::ADMIN
    }
}

#[system]
mod IsAuthorized {
    use traits::Into;
    use dojo_core::auth::components::{AuthStatus, AuthRole};
    use dojo_core::world::World;

    fn execute(ctx: Context, target_id: felt252, resource_id: felt252) -> bool {
        // Check if execution role is not set
        let scoped_role = if ctx.execution_role.id == 0.into() {
            // Use default component-scoped role
            // TODO: use commands once parsing is fixed
            let mut role = ctx
                .world
                .entity('AuthRole'.into(), (target_id, resource_id).into(), 0, 0);
            let scoped_role = serde::Serde::<AuthRole>::deserialize(ref role);
            match scoped_role {
                Option::Some(scoped_role) => scoped_role.id,
                Option::None(_) => 0.into(),
            }
        } else {
            // Use the set execution role
            ctx.execution_role.id
        };

        // Get authorization status for scoped role
        let maybe_authorization_status = commands::<AuthStatus>::try_entity(
            (scoped_role, resource_id).into()
        );
        let authorization_status = match maybe_authorization_status {
            Option::Some(authorization_status) => authorization_status.is_authorized,
            Option::None(_) => false,
        };

        // Check if system is authorized
        if authorization_status {
            return authorization_status;
        }

        // If system is not authorized, get World level role
        let role = commands::<AuthRole>::entity(target_id.into());

        // Check if system's role is Admin and executed by an Admin
        if role.id.into() == World::ADMIN {
            assert(ctx.execution_role.id.into() == World::ADMIN, 'Unauthorized Admin call');
            true
        } else {
            false
        }
    }
}

#[system]
mod GrantAuthRole {
    use traits::Into;
    use array::ArrayTrait;
    use dojo_core::auth::components::AuthRole;

    fn execute(target_id: felt252, role_id: felt252) {
        commands::set_entity(target_id.into(), (AuthRole { id: role_id }));
    }
}

#[system]
mod GrantScopedAuthRole {
    use traits::Into;
    use array::ArrayTrait;
    use dojo_core::auth::components::AuthRole;


    fn execute(target_id: felt252, role_id: felt252, resource_id: felt252) {
        commands::set_entity((target_id, resource_id).into(), (AuthRole { id: role_id }));
    }
}

#[system]
mod GrantResource {
    use traits::Into;
    use dojo_core::auth::components::AuthStatus;

    fn execute(role_id: felt252, resource_id: felt252) {
        commands::set_entity((role_id, resource_id).into(), (AuthStatus { is_authorized: true }));
    }
}

#[system]
mod RevokeAuthRole {
    use traits::Into;
    use array::ArrayTrait;
    use dojo_core::auth::components::AuthRole;

    fn execute(target_id: felt252) {
        commands::set_entity(target_id.into(), (AuthRole { id: 0.into() }));
    }
}

#[system]
mod RevokeScopedAuthRole {
    use traits::Into;
    use array::ArrayTrait;
    use dojo_core::auth::components::AuthRole;

    fn execute(target_id: felt252, resource_id: felt252) {
        commands::set_entity((target_id, resource_id).into(), (AuthRole { id: 0.into() }));
    }
}

#[system]
mod RevokeResource {
    use traits::Into;
    use dojo_core::auth::components::AuthStatus;

    fn execute(role_id: felt252, resource_id: felt252) {
        commands::set_entity((role_id, resource_id).into(), (AuthStatus { is_authorized: false }));
    }
}
