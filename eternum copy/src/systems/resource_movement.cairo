// Moves Bundle of resource from Point to Point

#[system]
mod ResourceMovementSystem {
    use array::ArrayTrait;
    use traits::Into;  

    use eternum::components::realms::Realm;

    fn execute(realm_id: felt252, location: Point) {

        // 1. Get location of Realm
        // 2. Calculate time it will take to move resource
        // 3. Set time
        // 4. Check enough resources in balance
        // 4. Find resource bundle slot - we need to make a component with a nested slot for resources
        // 5. Save resource bundle in slot
        // 7. Save arrival time

    }
}