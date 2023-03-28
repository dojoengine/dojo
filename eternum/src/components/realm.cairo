// Realm specific Metadata

use eternum::constants::TICK_TIME;
use starknet;

#[derive(Component)]
struct Realm {
    id: felt252, // OG Realm Id
    founder: felt252, // address of founder
    armies: Array::<i33>, // ??? TODO - We need a way to attach Armies to the Realm. This needs to be an array
    resource_ids: Array::<i33>, // ids of resources
    cities: felt252,
    harbors: felt252,
    rivers: felt252,
    regions: felt252,
}

trait RealmTrait {
    // calculates happiness on the Realm
    fn happiness(self: Realm, buildings: Buildings, army_population: felt252) -> felt252; 
}

impl RealmImpl of RealmTrait {
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

