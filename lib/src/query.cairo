use array::ArrayTrait;
use dojo::storage::PartitionKey;
use dojo::storage::StorageKey;

trait Query<T> {
    fn ids(partition: PartitionKey) -> Array::<felt252>;
    fn entity(key: StorageKey) -> T;
}
