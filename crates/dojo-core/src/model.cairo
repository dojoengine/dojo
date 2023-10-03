trait Model<T> {
    fn name(self: @T) -> felt252;
    fn keys(self: @T) -> Span<felt252>;
    fn keys_layout(self: @T) -> Span<u8>;
    fn values(self: @T) -> Span<felt252>;
    fn layout(self: @T) -> Span<u8>;
    fn packed_size(self: @T) -> usize;
}

#[starknet::interface]
trait IModel<T> {
    fn name(self: @T) -> felt252;
    fn layout(self: @T) -> Span<felt252>;
    fn schema(self: @T) -> Span<dojo::database::schema::Member>;
}
