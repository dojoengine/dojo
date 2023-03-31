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
    fn population(self: Buildings) -> felt252; 
}

impl BuildingsImpl of BuildingsTrait {
    fn population(self: Buildings) -> felt252 {
        // calculate building population
    }
}

