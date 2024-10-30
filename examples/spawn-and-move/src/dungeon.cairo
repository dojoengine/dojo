#[starknet::interface]
pub trait IDungeon<T> {
    fn enter(self: @T);
}

#[dojo::contract]
pub mod dungeon {
    #[abi(embed_v0)]
    pub impl IDungeonImpl of super::IDungeon<ContractState> {
        fn enter(self: @ContractState) {}
    }
}
