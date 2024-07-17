#[dojo::interface]
pub trait IDungeon {
    fn enter();
}

#[dojo::contract]
pub mod dungeon {
    #[abi(embed_v0)]
    pub impl IDungeonImpl of super::IDungeon<ContractState> {
        fn enter() {}
    }
}
