#[starknet::interface]
pub trait IHello<T> {
    fn say_hello(self: @T, name: ByteArray) -> ByteArray;
}

#[starknet::contract]
mod hello {
    #[storage]
    struct Storage {}

    #[abi(embed_v0)]
    impl HelloImpl of super::IHello<ContractState> {
        fn say_hello(self: @ContractState, name: ByteArray) -> ByteArray {
            format!("Hello, {name}!")
        }
    }
}
