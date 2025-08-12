#[derive(Drop, Serde)]
#[dojo::event]
pub struct EH {
    #[key]
    pub k: felt252,
    pub v: u32,
}

#[derive(Copy, Drop, Serde, IntrospectPacked, Debug)]
pub struct Vec2 {
    pub x: u32,
    pub y: u32,
}
