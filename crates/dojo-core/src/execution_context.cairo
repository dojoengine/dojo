use dojo_core::interfaces::IWorldDispatcher;
use dojo_core::auth::components::AuthRole;
use starknet::{ContractAddress, ClassHash};

#[derive(Copy, Drop, Serde)]
struct Context {
    world: IWorldDispatcher, // Dispatcher to the world contract
    caller_account: ContractAddress, // Address of the origin
    caller_system: ClassHash, // Class hash of the calling system
    caller_role: AuthRole, // Authorization role used for this call
}
