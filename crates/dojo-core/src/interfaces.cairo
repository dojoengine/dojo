use array::ArrayTrait;

#[starknet::interface]
trait IComponent<T> {
    fn name(self: @T) -> felt252;
}

#[starknet::interface]
trait ISystem<T> {
    fn name(self: @T) -> felt252;
    fn dependencies(self: @T) -> Array<(felt252, bool)>;
}
