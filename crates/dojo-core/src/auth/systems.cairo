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
    use array::ArrayTrait;

    use dojo_core::auth::components::Status;
    use dojo_core::auth::components::Role;
    use super::Route;

    use starknet::ContractAddress;

    fn execute(ref routing: Array<Route>) {
        _set_authorization_routing(ref routing, world_address);
    }

    fn _set_authorization_routing(ref routing: Array<Route>, world_address: starknet::ContractAddress) {
        gas::withdraw_gas().expect('Out of gas');

        if routing.is_empty() {
            return ();
        }

        let r = routing.pop_front().unwrap();

        let mut calldata = ArrayTrait::new(); 
        serde::Serde::<Role>::serialize(ref calldata, Role { id: r.role_id });
        IWorldDispatcher { contract_address: world_address }.set_entity(
            dojo_core::string::ShortStringTrait::new('Role'), QueryTrait::new_from_id(r.target_id), 0_u8, calldata.span());

        let mut calldata = ArrayTrait::new(); 
        serde::Serde::<Status>::serialize(ref calldata, Status { is_authorized: bool::True(()) });
        IWorldDispatcher { contract_address: world_address }.set_entity(
            dojo_core::string::ShortStringTrait::new('Status'), (r.role_id, r.resource_id).into(), 0_u8, calldata.span());

        _set_authorization_routing(ref routing, world_address);
    }
}

#[system]
mod Authorize {
    use traits::Into;
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

    use dojo_core::auth::components::Role;

    fn execute(target_id: u250, role_id: u250) {
        commands::set_entity(target_id.into(), (Role { id: role_id }));
    }
}

#[system]
mod GrantResource {
    use traits::Into;
    use dojo_core::auth::components::Status;

    fn execute(role_id: u250, resource_id: u250) {
        commands::set_entity((role_id, resource_id).into(), (bool::True(())));
    }
}

#[system]
mod RevokeRole {
    use traits::Into;
    use array::ArrayTrait;

    use dojo_core::auth::components::Role;

    fn execute(target_id: u250) {
        commands::set_entity(target_id.into(), (Role { id: 0.into() }));
    }
}

#[system]
mod RevokeResource {
    use traits::Into;
    use dojo_core::auth::components::Status;

    fn execute(role_id: u250, resource_id: u250) {
        commands::set_entity((role_id, resource_id).into(), (bool::False(())));
    }
}
