use dojo::interfaces::IWorldDispatcher;
use dojo::auth::components::AuthRole;
use starknet::{ContractAddress, ClassHash};

#[derive(Copy, Drop, Serde)]
struct Context {
    world: IWorldDispatcher, // Dispatcher to the world contract
    caller_account: ContractAddress, // Address of the origin
    caller_system: felt252, // Name of the calling system
    execution_role: AuthRole, // Authorization role used for this call
}
