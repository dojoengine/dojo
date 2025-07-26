use starknet::ContractAddress;

#[derive(Serde, Copy, Drop, Introspect, PartialEq, Debug, Default)]
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

#[derive(Drop, Serde)]
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

#[derive(Copy, Drop, Serde, Debug)]
#[dojo::model]
pub struct MockToken {
    #[key]
    pub account: ContractAddress,
    pub amount: u128,
}

#[derive(Copy, Drop, Serde, IntrospectPacked, Debug)]
pub struct Vec2 {
    pub x: u32,
    pub y: u32,
}

// If `Vec2` wasn't packed, the `Position` would be invalid,
// and a runtime error would be thrown.
// Any field that is a custom type into a `IntrospectPacked` type
// must be packed.
#[derive(Copy, Drop, Serde, IntrospectPacked, Debug)]
#[dojo::model]
pub struct Position {
    #[key]
    pub player: ContractAddress,
    pub vec: Vec2,
}

// Every field inside a model must derive `Introspect` or `IntrospectPacked`.
// `IntrospectPacked` can also be used into models that are only using `Introspect`.
#[derive(Copy, Drop, Serde, Introspect, PartialEq)]
pub struct PlayerItem {
    pub item_id: u32,
    pub quantity: u32,
    pub score: i32,
}

#[derive(Drop, Serde, Introspect)]
pub struct PlayerConfigItems {
    pub items: Array<PlayerItem>,
    pub favorite_item: Option<u32>,
}

#[derive(Drop, Serde)]
#[dojo::model]
pub struct PlayerConfig {
    #[key]
    pub player: ContractAddress,
    pub name: ByteArray,
    pub items: Array<PlayerItem>,
    pub favorite_item: Option<u32>,
}

#[derive(Drop, Serde)]
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
