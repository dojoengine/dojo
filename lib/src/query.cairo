use array::ArrayTrait;

trait Query<T> {
    fn ids() -> Array::<usize>;
    fn entity(path: T) -> T;
    fn len() -> u32;
}
