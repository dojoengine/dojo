trait Component<T> {
    fn name(self: @T) -> felt252;
    fn keys(self: @T) -> Span<felt252>;
    fn values(self: @T) -> Span<felt252>;
    fn layout(self: @T) -> Span<u8>;
}

#[starknet::interface]
trait IComponent<T> {
    fn name(self: @T) -> felt252;
    fn layout(self: @T) -> Span<felt252>;
    fn schema(self: @T) -> Span<dojo::database::schema::Member>;
}

#[starknet::interface]
trait ISystem<T> {
    fn name(self: @T) -> felt252;
}
