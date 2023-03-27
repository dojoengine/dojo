// Creates a tick system which is called before Realm state change

#[system]
mod SettleSystem {
    use array::ArrayTrait;
    use traits::Into;  

    use eternum::components::realms::Realm;

    fn execute(realm_id: felt252) {

        // executes state updates across the Realms

        // update Armies
        // decay buildings
        // ....

    }
}