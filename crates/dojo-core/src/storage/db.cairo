#[contract]
mod Database {
    use array::{ArrayTrait, SpanTrait};
    use traits::{Into, TryInto};
    use serde::Serde;
    use hash::LegacyHash;
    use poseidon::poseidon_hash_span;

    use dojo_core::serde::SpanSerde;
    use dojo_core::storage::{index::Index, kv::KeyValueStore, query::{Query, QueryTrait, QueryIntoFelt252}};
    use dojo_core::integer::{u250, Felt252IntoU250};
    use dojo_core::interfaces::{IComponentLibraryDispatcher, IComponentDispatcherTrait};

    #[event]
    fn StoreSetRecord(table_id: u250, keys: Span<u250>, value: Span<felt252>) {}

    #[event]
    fn StoreSetField(table_id: u250, keys: Span<u250>, offset: u8, value: Span<felt252>) {}

    #[event]
    fn StoreDeleteRecord(tableId: u250, keys: Span<u250>) {}

    fn get(
        class_hash: starknet::ClassHash, table: u250, query: Query, offset: u8, length: usize
    ) -> Option<Span<felt252>> {
        let mut length = length;
        if length == 0 {
            length = IComponentLibraryDispatcher { class_hash: class_hash }.len();
        }

        let id = query.id();
        match Index::exists(table, id) {
            bool::False(()) => Option::None(()),
            bool::True(()) => Option::Some(KeyValueStore::get(table, id, offset, length))
        }
    }

    fn set(
        class_hash: starknet::ClassHash, table: u250, query: Query, offset: u8, value: Span<felt252>
    ) {
        let keys = query.keys();
        let id = query.into();

        let length = IComponentLibraryDispatcher { class_hash: class_hash }.len();
        assert(value.len() <= length, 'Value too long');

        Index::create(table, id);
        KeyValueStore::set(table, id, offset, value);

        StoreSetRecord(table, keys, value);
        StoreSetField(table, keys, offset, value);
    }

    fn del(class_hash: starknet::ClassHash, table: u250, query: Query) {
        Index::delete(table, query.into());
    }

    fn all(component: u250, partition: u250) -> Array<u250> {
        if partition == 0.into() {
            return Index::query(component);
        }

        let mut serialized = ArrayTrait::new();
        component.serialize(ref serialized);
        partition.serialize(ref serialized);
        let hash = poseidon_hash_span(serialized.span());
        Index::query(hash.into())
    }
}
