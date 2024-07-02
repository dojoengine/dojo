use starknet::ContractAddress;

#[derive(Serde, Copy, Drop, Introspect, PartialEq)]
enum Direction {
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
struct Message {
    #[key]
    identity: ContractAddress,
    #[key]
    channel: felt252,
    message: ByteArray,
    #[key]
    salt: felt252
}

#[derive(Copy, Drop, Serde)]
#[dojo::model]
struct Moves {
    #[key]
    player: ContractAddress,
    remaining: u8,
    last_direction: Direction
}

#[derive(Copy, Drop, Serde)]
#[dojo::model]
struct MockToken {
    #[key]
    account: ContractAddress,
    amount: u128,
}

#[derive(Copy, Drop, Serde, IntrospectPacked)]
struct Vec2 {
    x: u32,
    y: u32
}

// If `Vec2` wasn't packed, the `Position` would be invalid,
// and a runtime error would be thrown.
// Any field that is a custom type into a `IntrospectPacked` type
// must be packed.
#[derive(Copy, Drop, Serde, IntrospectPacked)]
#[dojo::model]
struct Position {
    #[key]
    player: ContractAddress,
    vec: Vec2,
}

// Every field inside a model must derive `Introspect` or `IntrospectPacked`.
// `IntrospectPacked` can also be used into models that are only using `Introspect`.
#[derive(Copy, Drop, Serde, Introspect)]
struct PlayerItem {
    item_id: u32,
    quantity: u32,
}

#[derive(Drop, Serde)]
#[dojo::model]
struct PlayerConfig {
    #[key]
    player: ContractAddress,
    name: ByteArray,
    items: Array<PlayerItem>,
    favorite_item: Option<u32>,
}

#[derive(Drop, Serde)]
#[dojo::model]
struct ServerProfile {
    #[key]
    player: ContractAddress,
    #[key]
    server_id: u32,
    name: ByteArray,
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
    use super::{Position, Vec2, Vec2Trait};

    #[test]
    #[available_gas(100000)]
    fn test_vec_is_zero() {
        assert(Vec2Trait::is_zero(Vec2 { x: 0, y: 0 }), 'not zero');
    }

    #[test]
    #[available_gas(100000)]
    fn test_vec_is_equal() {
        let position = Vec2 { x: 420, y: 0 };
        assert(position.is_equal(Vec2 { x: 420, y: 0 }), 'not equal');
    }
}
