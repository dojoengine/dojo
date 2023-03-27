#[derive(Component)]
struct Realm {
    settled_time: felt252, // settled owner
    founded: felt252 // address of founder
}

#[derive(Component)]
struct RealmBuildings {
    barracks: felt252,
    castle: felt252,
    archer_tower: felt252,
    mage_tower: felt252,
}

// Realms can have multiple Armies attached
#[derive(Component)]
struct RealmArmy {
    army_id: felt252,
    light_cavalry: felt252,
    heavy_cavalry: felt252,
    archer: felt252,
    longbow: felt252,
    mage: felt252,
    arcanist: felt252,
    light_infantry: felt252,
    heavy_infantry: felt252,
    positions: Point // location of Army
}
