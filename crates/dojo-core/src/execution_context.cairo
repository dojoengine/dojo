use dojo_core::interfaces::IWorldDispatcher;
use dojo_core::auth::components::AuthRole;
use dojo_core::string::ShortString;
use starknet::{ContractAddress, ClassHash};

#[derive(Copy, Drop, Serde)]
struct Context {
    world: IWorldDispatcher, // Dispatcher to the world contract
    caller_account: ContractAddress, // Address of the origin
    caller_system: ShortString, // Name of the calling system
    execution_role: AuthRole, // Authorization role used for this call
}
