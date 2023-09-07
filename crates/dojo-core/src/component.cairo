trait Component<T> {
    fn name(self: @T) -> felt252;
    fn keys(self: @T) -> Span<felt252>;
    fn pack(self: @T) -> Span<felt252>;
    fn unpack(ref packed: Span<felt252>) -> Option<T>;
}

#[starknet::interface]
trait IComponent<T> {
    fn name(self: @T) -> felt252;
    fn layout(self: @T) -> Span<felt252>;
    fn schema(self: @T) -> Array<(felt252, felt252, bool)>;
}

#[starknet::interface]
trait ISystem<T> {
    fn name(self: @T) -> felt252;
}
