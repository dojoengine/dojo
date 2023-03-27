use constants::TICK_TIME;
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

//
// ---------- Buildings
// NB: Have left this open and not specifcially tied to only Realms. Barbarians could share the same buildings.

#[derive(Component)]
struct Buildings {
    barracks: felt252,
    castle: felt252,
    archer_tower: felt252,
    mage_tower: felt252,
    store_house: felt252
}

trait BuildingsTrait {
    // population
    fn population(self: Realm, buildings: Buildings) -> felt252; 
}

impl BuildingsImpl of BuildingsTrait {
    fn population(self: Buildings) -> felt252 {
        // calculate building population
    }
}

