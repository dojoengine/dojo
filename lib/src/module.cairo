use hash::LegacyHash;
use serde::Serde;
use traits::Into;

#[derive(Copy, Drop)]
struct ModuleID {
    kind: u8,
    name: felt252,
}

trait ModuleIDTrait {
    fn new(kind: u8, name: felt252) -> ModuleID;
    fn hash(self: @ModuleID) -> felt252;
}

impl ModuleIDImpl of ModuleIDTrait {
    fn new(kind: u8, name: felt252) -> ModuleID {
        ModuleID { kind: kind, name: name,  }
    }

    fn hash(self: @ModuleID) -> felt252 {
        pedersen((*self.kind).into(), *self.name)
    }
}

impl LegacyHashModuleID of LegacyHash::<ModuleID> {
    fn hash(state: felt252, key: ModuleID) -> felt252 {
        LegacyHash::hash(state, key.hash())
    }
}

impl ModuleIDSerde of serde::Serde::<ModuleID> {
    fn serialize(ref serialized: Array::<felt252>, input: ModuleID) {
        Serde::<u8>::serialize(ref serialized, input.kind);
        Serde::<felt252>::serialize(ref serialized, input.name);
    }
    fn deserialize(ref serialized: Span::<felt252>) -> Option::<ModuleID> {
        let kind = Serde::<u8>::deserialize(ref serialized)?;
        let name = Serde::<felt252>::deserialize(ref serialized)?;
        Option::Some(ModuleID { kind: kind, name: name })
    }
}
