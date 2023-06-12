#[contract]
mod Database {
    use array::{ArrayTrait, SpanTrait};
    use traits::{Into, TryInto};
    use serde::Serde;
    use hash::LegacyHash;
    use option::OptionTrait;
    use poseidon::poseidon_hash_span;

    use dojo_core::serde::SpanSerde;
    use dojo_core::storage::{index::Index, kv::KeyValueStore, key::{Column, Key, KeyTrait}};
    use dojo_core::interfaces::{IComponentLibraryDispatcher, IComponentDispatcherTrait};

    #[event]
    fn StoreSetRecord(table_id: felt252, columns: Span<Column>, offset: u8, value: Span<felt252>) {}

    #[event]
    fn StoreDeleteRecord(table_id: felt252, columns: Span<Column>) {}

    fn get(
        class_hash: starknet::ClassHash, table: felt252, key: Key, offset: u8, length: usize
    ) -> Option<Span<felt252>> {
        let mut length = length;
        if length == 0 {
            length = IComponentLibraryDispatcher { class_hash: class_hash }.len();
        }

        let id = key.hash();
        match Index::exists(table, id) {
            bool::False(()) => Option::None(()),
            bool::True(()) => Option::Some(KeyValueStore::get(table, id, offset, length))
        }
    }

    fn set(
        class_hash: starknet::ClassHash, table: felt252, key: Key, offset: u8, value: Span<felt252>
    ) {
        let columns = key.columns();
        let id = key.hash();

        let length = IComponentLibraryDispatcher { class_hash: class_hash }.len();
        assert(value.len() <= length, 'Value too long');

        Index::create(table, id);
        KeyValueStore::set(table, id, offset, value);

        StoreSetRecord(table, columns, offset, value);
    }

    fn del(class_hash: starknet::ClassHash, table: felt252, key: Key) {
        Index::delete(table, key.hash());
        StoreDeleteRecord(table, key.columns());
    }

    // returns a tuple of spans, first contains the entity IDs,
    // second the deserialized entities themselves
    fn all(
        class_hash: starknet::ClassHash, table: felt252, index: felt252
    ) -> (Span<felt252>, Span<Span<felt252>>) {
        let all_ids = Index::query(index);
        let length = IComponentLibraryDispatcher { class_hash: class_hash }.len();

        let mut ids = all_ids.span();
        let mut entities: Array<Span<felt252>> = ArrayTrait::new();
        loop {
            match ids.pop_front() {
                Option::Some(id) => {
                    let value: Span<felt252> = KeyValueStore::get(table, *id, 0_u8, length);
                    entities.append(value);
                },
                Option::None(_) => {
                    break (all_ids.span(), entities.span());
                }
            };
        }
    }
}
