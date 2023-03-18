use dojo::storage::StorageKey;

trait Spawn<T> {
    fn bundle(key: StorageKey, bundle: T) -> usize;
}
