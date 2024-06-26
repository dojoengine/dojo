#[starknet::interface]
trait ISystem<T> {
    fn name(self: @T) -> ByteArray;
    fn selector(self: @T) -> felt252;

    /// Returns the namespace of a system.
    fn namespace(self: @T) -> ByteArray;

    // Returns the namespace selector built from its name.
    /// namespace_selector = hash(namespace_name)
    fn namespace_selector(self: @T) -> felt252;
}
