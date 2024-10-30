#[starknet::interface]
pub trait IContract<T> {
    fn dojo_name(self: @T) -> ByteArray;
}
