use starknet::{ContractAddress, get_caller_address};

// TODO import all this when complex benchmarks are merged
#[derive(Model, Copy, Drop, Serde)]
struct Character {
    #[key]
    caller: ContractAddress,
    heigth: felt252,
    abilities: Abilities,
    stats: Stats,
    weapon: Weapon,
    gold: u32,
}

#[derive(Introspect, Copy, Drop, Serde)]
struct Abilities {
    strength: u8,
    dexterity: u8,
    constitution: u8,
    intelligence: u8,
    wisdom: u8,
    charisma: u8,
}

#[derive(Introspect, Copy, Drop, Serde)]
struct Stats {
    kills: u128,
    deaths: u16,
    rests: u32,
    hits: u64,
    blocks: u32,
    walked: felt252,
    runned: felt252,
    finished: bool,
    romances: u16,
}

#[derive(Introspect, Copy, Drop, Serde)]
enum Weapon {
    DualWield: (Sword, Sword),
    Fists: (Sword, Sword), // Introspect requires same arms
}

#[derive(Introspect, Copy, Drop, Serde)]
struct Sword {
    swordsmith: ContractAddress,
    damage: u32,
}

#[derive(Model, Copy, Drop, Serde)]
struct Case {
    #[key]
    owner: ContractAddress,
    sword: Sword,
    material: felt252,
}

#[derive(Model, Copy, Drop, Serde)]
struct Alias {
    #[key]
    player: ContractAddress,
    name: felt252,
}