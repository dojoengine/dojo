use array::ArrayTrait;

#[contract]
mod Index {
    use array::ArrayTrait;
    use array::SpanTrait;
    use traits::Into;

    struct Storage {
        // Maps id to it's position in the table.
        // NOTE: ids is 1-indexed to allow for 0
        // to be used as a sentinel value.
        ids: LegacyMap::<(felt252, felt252), usize>,
        table_lens: LegacyMap::<felt252, usize>,
        tables: LegacyMap::<(felt252, usize), felt252>,
    }

    #[external]
    fn index(table: felt252, id: felt252) {
        let is_set = ids::read((table, id));
        if is_set != 0_usize {
            return ();
        }

        let table_len = table_lens::read(table);
        ids::write((table, id), table_len + 1_usize);
        table_lens::write(table, table_len + 1_usize);
        tables::write((table, table_len), id);
    }

    #[external]
    fn delete(table: felt252, id: felt252) {
        let table_len = table_lens::read(table);
        let table_idx = ids::read((table, id)) - 1_usize;
        ids::write((table, id), 0_usize);
        table_lens::write(table, table_len - 1_usize);

        // Replace the deleted element with the last element.
        // NOTE: We leave the last element set as to not produce an unncessary state diff.
        tables::write((table, table_idx), tables::read((table, table_len - 1_usize)));
    }

    #[view]
    fn exists(table: felt252, id: felt252) -> bool {
        ids::read((table, id)) != 0_usize
    }

    #[view]
    fn records(table: felt252) -> Array::<felt252> {
        let mut res = ArrayTrait::<felt252>::new();
        let table_len = table_lens::read(table);
        _records(table, 0_usize, table_len, ref res);
        res
    }

    fn _records(table: felt252, idx: usize, table_len: usize, ref res: Array::<felt252>) {
        match gas::withdraw_gas_all(get_builtin_costs()) {
            Option::Some(_) => {},
            Option::None(_) => {
                let mut data = ArrayTrait::new();
                data.append('Out of gas');
                panic(data);
            },
        }

        if (idx == table_len) {
            return ();
        }

        res.append(tables::read((table, idx)));
        return _records(table, idx + 1_usize, table_len, ref res);
    }
}

#[test]
#[available_gas(2000000)]
fn test_index_entity() {
    let no_records = Index::records(69);
    assert(no_records.len() == 0_usize, 'entity indexed');

    Index::index(69, 420);
    let records = Index::records(69);
    assert(records.len() == 1_usize, 'entity not indexed');
    assert(*records.at(0_usize) == 420, 'entity value incorrect');

    Index::index(69, 420);
    let noop_records = Index::records(69);
    assert(noop_records.len() == 1_usize, 'index should be noop');

    Index::index(69, 1337);
    let two_records = Index::records(69);
    assert(two_records.len() == 2_usize, 'index should have two records');
    assert(*two_records.at(1_usize) == 1337, 'entity value incorrect');
}

#[test]
#[available_gas(2000000)]
fn test_entity_delete() {
    Index::index(69, 420);
    let records = Index::records(69);
    assert(records.len() == 1_usize, 'entity not indexed');
    assert(*records.at(0_usize) == 420, 'entity value incorrect');

    assert(Index::exists(69, 420), 'entity should exist');

    Index::delete(69, 420);

    assert(!Index::exists(69, 420), 'entity should not exist');
    let no_records = Index::records(69);
    assert(no_records.len() == 0_usize, 'index should have no records');
}
