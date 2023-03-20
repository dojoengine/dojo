use array::ArrayTrait;
use dojo::storage::StorageKey;

trait Query<T> {
    fn ids(partition: felt252) -> Array::<felt252>;
    fn entity(key: StorageKey) -> T;
}
