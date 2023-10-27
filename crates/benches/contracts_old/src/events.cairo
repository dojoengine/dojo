use starknet::ContractAddress;

#[derive(Drop, Clone, Serde, PartialEq, starknet::Event)]
struct Moved {
    player: ContractAddress,
    x: u32,
    y: u32
}

#[derive(Drop, Clone, Serde, PartialEq, starknet::Event)]
enum Event {
    Moved: Moved
}
