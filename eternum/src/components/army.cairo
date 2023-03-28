use eternum::components::buildings::Buildings;

//
// ----------- Army
// Armies can be attacked to any Entity if it has a Spawn function. Realms can spawn them and have ownership over them, however they can exist
// outside of a Realm. They might spawn randomly on the Map.

#[derive(Component)]
struct Army {
    owner_entity_id: u32, // Armies are Entities. We assign an Army to a Realm for example, to give ownership to the Realm.
    light_cavalry_qty: u32,
    light_cavalry_health: u32,
    heavy_cavalry_qty: u32,
    heavy_cavalry_health: u32,
    archer_qty: u32,
    archer_health: u32,
    longbow_qty: u32,
    longbow_health: u32,
    mage_qty: u32,
    mage_health: u32,
    arcanist_qty: u32,
    arcanist_health: u32,
    light_infantry_qty: u32,
    light_infantry_health: u32,
    heavy_infantry_qty: u32,
    heavy_infantry_health: u32,
}

// Holds statistics for Army used in calculations
#[derive(Copy, Drop)]
struct ArmyStatistics {
    cavalry_attack: u32,
    archery_attack: u32,
    magic_attack: u32,
    infantry_attack: u32,
    cavalry_defence: u32,
    archery_defence: u32,
    magic_defence: u32,
    infantry_defence: u32,
}

trait ArmyTrait {
    // population
    fn population(self: Army) -> felt252;

    // calculates statistics of Army
    fn statistics(self: Army) -> felt252;
}

impl ArmyImpl of ArmyTrait {
    fn population(self: Army) -> felt252 {
        // recurse through Armies of Realm
        0
    }
    fn statistics(self: Army) -> felt252 {
        // calcs statistics
        0
    }
}
