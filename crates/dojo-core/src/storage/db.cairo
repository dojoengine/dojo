#[contract]
mod Database {
    use array::ArrayTrait;
    use array::SpanTrait;
    use traits::Into;

    use dojo_core::serde::SpanSerde;

    use dojo_core::storage::key::StorageKey;
    use dojo_core::storage::key::StorageKeyTrait;
    use dojo_core::storage::key::StorageKeyIntoFelt252;
    use dojo_core::storage::kv::KeyValueStore;
    use dojo_core::storage::indexer::Indexer;

    #[event]
    fn StoreSetRecord(table_id: felt252, key: Span<felt252>, value: Span<felt252>) {}

    #[event]
    fn StoreSetField(table_id: felt252, key: Span<felt252>, offset: u8, value: Span<felt252>) {}

    #[event]
    fn StoreDeleteRecord(tableId: felt252, key: Span<felt252>) {}

    #[view]
    fn get(
        class_hash: starknet::ClassHash,
        table: felt252,
        key: StorageKey,
        offset: u8,
        mut length: usize
    ) -> Span<felt252> {
        KeyValueStore::get(table, key.into(), offset, length)
    }

    #[external]
    fn set(
        class_hash: starknet::ClassHash,
        table: felt252,
        key: StorageKey,
        offset: u8,
        value: Span<felt252>
    ) {
        let id = key.into();
        // let keys = key.keys();
        Indexer::index(table, id);
        KeyValueStore::set(table, id, offset, value);

        // StoreSetRecord(table, keys, value);
        // StoreSetField(table, keys, offset, value);
    }

    #[external]
    fn del(class_hash: starknet::ClassHash, table: felt252, key: StorageKey) {
        
    }

    fn all(component: felt252, partition: felt252) -> Array::<felt252> {
        if partition == 0 {
            return Indexer::records(component);
        }

        Indexer::records(pedersen(component, partition))
    }
}
