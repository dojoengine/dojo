use eternum::constants::TICK_TIME;
use starknet;

#[derive(Component)]
struct Realm {
    id: felt252, // OG Realm Id
    settled_time: felt252, // settled owner
    founded: felt252, // address of founder
    last_update: felt252, // address of founder
    armies: felt252 // ??? TODO - We need a way to attach Armies to the Realm. This needs to be an array
}

trait RealmTrait {
    // check last update in the past
    fn tick(self: Realm) -> bool;

    // calculates happiness on the Realm
    fn happiness(self: Realm, buildings: Buildings, army_population: felt252) -> felt252; 
}

impl RealmImpl of RealmTrait {
    fn tick(self: Realm) -> bool {
        let info = starknet::get_block_info().unbox();

        if (self.last_update + TICK_TIME) < info.block_timestamp {
            true
        } else {
            false
        }
    }
    fn happiness(self: Realm, buildings: Buildings) -> felt252 {
        // calculate happiness
        // return happiness
    }

    fn population(self: Realm, building_population: felt252, army_population: felt252) -> felt252 {
        // calculate building population
        // calculate army population
        // return population
    }
}

