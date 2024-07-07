#[starknet::interface]
trait IContract<T> {
    fn contract_name(self: @T) -> ByteArray;
    fn selector(self: @T) -> felt252;

    /// Returns the namespace of a contract.
    fn namespace(self: @T) -> ByteArray;

    // Returns the namespace selector built from its name.
    /// namespace_selector = hash(namespace_name)
    fn namespace_selector(self: @T) -> felt252;

    // Returns the contract tag
    fn tag(self: @T) -> ByteArray;
}
