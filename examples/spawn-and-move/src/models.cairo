use starknet::ContractAddress;

// as EnemyType is used as model key only, it does not need DojoStore and Default traits
// whatever if it's for a new or a legacy model.
#[derive(Serde, Copy, Drop, Debug, PartialEq, Introspect)]
pub enum EnemyType {
    Goblin,
    Orc,
    Troll,
    Dragon,
    Giant,
}

// as EnemySpeed is used as model value for new and legacy models,
// it needs DojoStore and Default traits.
#[derive(Serde, Copy, Drop, Introspect, DojoStore, Default)]
pub enum EnemySpeed {
    #[default]
    Slow,
    Medium,
    High,
}

#[derive(Serde, Copy, Drop, Introspect, DojoStore)]
pub enum EnemyProperty {
    HealthBooster: u8,
    SpecialAttack: u8,
    Shield: Option<u8>,
    SpeedBooster: EnemySpeed,
}

// manually implement Default trait for EnemyProperty to configure
// the default health value.
impl EnemyPropertyDefault of Default<EnemyProperty> {
    fn default() -> EnemyProperty {
        EnemyProperty::HealthBooster(100)
    }
}

#[dojo::model]
pub struct Enemy {
    #[key]
    pub enemy_type: EnemyType,
    pub properties: Array<EnemyProperty>,
}

// as VintageEnemyCharacteristics is used as model value for legacy models only,
// it does not need DojoStore trait.
#[derive(Serde, Copy, Drop, Debug, PartialEq, Introspect)]
pub struct VintageEnemyCharacteristics {
    pub attack: u8,
    pub defense: u8,
    pub shield: Option<u8>,
}

// A legacy model to showcase how to safely use enums and options with legacy models.
#[derive(DojoLegacyStore)]
#[dojo::model]
pub struct VintageEnemy {
    #[key]
    pub enemy_type: EnemyType,
    pub characteristics: VintageEnemyCharacteristics,
    pub speed: EnemySpeed,
    pub is_set: bool,
}

#[derive(Serde, Copy, Drop, Introspect, DojoStore, PartialEq, Debug, Default)]
pub enum Direction {
    #[default]
    None,
    Left,
    Right,
    Up,
    Down,
}

impl DirectionIntoFelt252 of Into<Direction, felt252> {
    fn into(self: Direction) -> felt252 {
        match self {
            Direction::None => 0,
            Direction::Left => 1,
            Direction::Right => 2,
            Direction::Up => 3,
            Direction::Down => 4,
        }
    }
}

#[dojo::model]
pub struct Message {
    #[key]
    pub identity: ContractAddress,
    #[key]
    pub channel: felt252,
    #[key]
    pub salt: felt252,
    pub message: ByteArray,
}

#[derive(Copy, Drop, Serde, Debug)]
#[dojo::model]
pub struct Moves {
    #[key]
    pub player: ContractAddress,
    pub remaining: u8,
    pub last_direction: Direction,
}

#[derive(Copy, Debug)]
#[dojo::model]
pub struct MockToken {
    #[key]
    pub account: ContractAddress,
    pub amount: u128,
}

#[derive(Copy, Drop, Serde, IntrospectPacked, DojoStore, Debug)]
pub struct Vec2 {
    pub x: u32,
    pub y: u32,
}

// If `Vec2` wasn't packed, the `Position` would be invalid,
// and a runtime error would be thrown.
// Any field that is a custom type into a `IntrospectPacked` type
// must be packed.
#[derive(Copy, IntrospectPacked, Debug)]
#[dojo::model]
pub struct Position {
    #[key]
    pub player: ContractAddress,
    pub vec: Vec2,
}

// Every field inside a model must derive `Introspect` or `IntrospectPacked`.
// `IntrospectPacked` can also be used into models that are only using `Introspect`.
#[derive(Copy, Drop, Serde, Introspect, DojoStore, PartialEq)]
pub struct PlayerItem {
    pub item_id: u32,
    pub quantity: u32,
    pub score: i32,
}

#[dojo::model]
pub struct PlayerConfig {
    #[key]
    pub player: ContractAddress,
    pub name: ByteArray,
    pub items: Array<PlayerItem>,
    pub favorite_item: Option<u32>,
}

#[dojo::model]
pub struct ServerProfile {
    #[key]
    pub player: ContractAddress,
    #[key]
    pub server_id: u32,
    pub name: ByteArray,
}

trait Vec2Trait {
    fn is_zero(self: Vec2) -> bool;
    fn is_equal(self: Vec2, b: Vec2) -> bool;
}

impl Vec2Impl of Vec2Trait {
    fn is_zero(self: Vec2) -> bool {
        if self.x - self.y == 0 {
            return true;
        }
        false
    }

    fn is_equal(self: Vec2, b: Vec2) -> bool {
        self.x == b.x && self.y == b.y
    }
}

#[cfg(test)]
mod tests {
    use super::{Vec2, Vec2Trait};

    #[test]
    #[available_gas(l2_gas: 100000)]
    fn test_vec_is_zero() {
        assert(Vec2Trait::is_zero(Vec2 { x: 0, y: 0 }), 'not zero');
    }

    #[test]
    #[available_gas(l2_gas: 100000)]
    fn test_vec_is_equal() {
        let position = Vec2 { x: 420, y: 0 };
        assert(position.is_equal(Vec2 { x: 420, y: 0 }), 'not equal');
    }
}
