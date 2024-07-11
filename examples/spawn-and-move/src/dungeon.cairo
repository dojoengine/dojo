#[dojo::interface]
trait IDungeon {
    fn enter();
}

#[dojo::contract]
mod dungeon {
    #[abi(embed_v0)]
    impl IDungeonImpl of super::IDungeon<ContractState> {
        fn enter() {}
    }
}
