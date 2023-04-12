#[system]
mod RouteAuth {
    use array::ArrayTrait;
    use traits::Into;

    use starknet::ContractAddress;

    use dojo_core::auth::components::Status;
    use dojo_core::auth::components::Role;
    use dojo_core::auth::types::Route;

    fn execute(ref routing: Array<Route>) {
        _set_authorization_routing(ref routing, world_address);
    }

    fn _set_authorization_routing(ref routing: Array<Route>, world_address: ContractAddress) {
        if routing.is_empty() {
            return ();
        }

        let r = routing.pop_front().unwrap();

        let mut calldata = ArrayTrait::new(); 
        serde::Serde::<Role>::serialize(ref calldata, Role { id: r.role_id });
        IWorldDispatcher { contract_address: world_address }.set_entity('Role', r.target_id.into(), 0_u8, calldata.span());

        let mut calldata = ArrayTrait::new(); 
        serde::Serde::<Status>::serialize(ref calldata, Status { is_authorized: bool::True(()) });
        IWorldDispatcher { contract_address: world_address }.set_entity('Status', (r.role_id, r.resource_id).into(), 0_u8, calldata.span());

        _set_authorization_routing(ref routing, world_address);
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
