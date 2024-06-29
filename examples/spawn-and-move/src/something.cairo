#[dojo::interface]
trait ISomething {
    fn something();
}

#[dojo::contract]
mod something {
    #[abi(embed_v0)]
    impl ISomethingImpl of super::ISomething<ContractState> {
        fn something() {}
    }
}
