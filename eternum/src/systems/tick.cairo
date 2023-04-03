// Creates a tick system which is called before Realm state change

#[system]
mod TickSystem {
    use array::ArrayTrait;
    use traits::Into;

    use eternum::components::tick::Tick;

    fn execute(realm_id: felt252) { // auth function - can only be called by approved systems
    // Can only be approved modules

    // Adjust state on Realm

    }
}
