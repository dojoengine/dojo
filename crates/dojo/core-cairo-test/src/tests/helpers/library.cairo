#[starknet::interface]
pub trait LibraryA<T> {
    fn get_byte(self: @T) -> u8;
}

#[dojo::library]
pub mod library_a {
    use super::LibraryA;

    #[abi(embed_v0)]
    impl LibraryAImpl of LibraryA<ContractState> {
        fn get_byte(self: @ContractState) -> u8 {
            42
        }
    }
}
