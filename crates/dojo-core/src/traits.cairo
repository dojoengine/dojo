trait Component<T> {
    fn name(self: @T) -> felt252;
    fn keys(self: @T) -> Span<felt252>;
    fn values(self: @T) -> Span<felt252>;
}

#[starknet::interface]
trait INamed<T> {
    fn name(self: @T) -> felt252;
}
