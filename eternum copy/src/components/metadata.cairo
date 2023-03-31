// stores entity metadata that is global to the world
// eg: birth time, moveable, alive

use eternum::constants::TICK_TIME;
use starknet;

#[derive(Component)]
struct MetaData {
    name: felt252,
    creation_timestamp: felt252,
    moveable: bool, // can move
    alive: bool,
    parent_entity: felt252, // if entity has parent
    last_update: felt252
}

trait RealmTrait {
    // check last update in the past
    fn tick(self: Realm) -> bool;
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
}

