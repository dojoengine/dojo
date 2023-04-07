#[contract]
mod Database {
    use array::ArrayTrait;
    use array::SpanTrait;
    use traits::Into;

    use dojo_core::serde::SpanSerde;

    use dojo_core::storage::query::Query;
    use dojo_core::storage::query::QueryTrait;
    use dojo_core::storage::query::QueryIntoFelt252;
    use dojo_core::storage::kv::KeyValueStore;
    use dojo_core::storage::index::Index;

    #[event]
    fn StoreSetRecord(table_id: felt252, keys: Span<felt252>, value: Span<felt252>) {}

    #[event]
    fn StoreSetField(table_id: felt252, keys: Span<felt252>, offset: u8, value: Span<felt252>) {}

    #[event]
    fn StoreDeleteRecord(tableId: felt252, keys: Span<felt252>) {}

    #[view]
    fn get(
        class_hash: starknet::ClassHash,
        table: felt252,
        query: Query,
        offset: u8,
        mut length: usize
    ) -> Span<felt252> {
        KeyValueStore::get(table, query.into(), offset, length)
    }

    #[external]
    fn set(
        class_hash: starknet::ClassHash,
        table: felt252,
        query: Query,
        offset: u8,
        value: Span<felt252>
    ) {
        let id = query.into();
        // let keys = query.keys();
        Index::index(table, id);
        KeyValueStore::set(table, id, offset, value);

        // StoreSetRecord(table, keys, value);
        // StoreSetField(table, keys, offset, value);
    }

    #[external]
    fn del(class_hash: starknet::ClassHash, table: felt252, query: Query) {
        
    }

    fn all(component: felt252, partition: felt252) -> Array::<felt252> {
        if partition == 0 {
            return Index::records(component);
        }

        Index::records(pedersen(component, partition))
    }
}
