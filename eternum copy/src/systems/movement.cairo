// Moves non-resource entites

#[system]
mod ResourceMovementSystem {
    use array::ArrayTrait;
    use traits::Into;

    use eternum::components::realms::Realm;

    fn execute(destination: felt252) {
        
        // 1. Check Entity can Move
        // 2. Get location of Entity
        // 3. Calculate distance of Entity
        // 4. Save travel time in asset

        // ? need system to check Entity has Arrived at location

    }
}

