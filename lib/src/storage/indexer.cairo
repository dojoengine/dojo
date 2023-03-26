use array::ArrayTrait;
use debug::PrintTrait;

#[contract]
mod Indexer {
    use array::ArrayTrait;
    use array::SpanTrait;
    use traits::Into;
    use debug::PrintTrait;

    struct Storage {
        ids: LegacyMap::<(felt252, felt252), bool>,
        table_lens: LegacyMap::<felt252, usize>,
        tables: LegacyMap::<(felt252, usize), felt252>,
    }

    #[external]
    fn index(table: felt252, id: felt252) {
        let is_set = ids::read((table, id));
        if is_set {
            return ();
        }

        ids::write((table, id), bool::True(()));
        let table_len = table_lens::read(table);
        table_lens::write(table, table_len + 1_usize);
        tables::write((table, table_len), id);
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
    let no_records = Indexer::records(69);
    assert(no_records.len() == 0_usize, 'entity indexed');

    Indexer::index(69, 420);
    let records = Indexer::records(69);
    assert(records.len() == 1_usize, 'entity not indexed');
    assert(*records.at(0_usize) == 420, 'entity value incorrect');

    Indexer::index(69, 420);
    let noop_records = Indexer::records(69);
    assert(noop_records.len() == 1_usize, 'index should be noop');

    Indexer::index(69, 1337);
    let two_records = Indexer::records(69);
    assert(two_records.len() == 2_usize, 'index should have two records');
    assert(*two_records.at(1_usize) == 1337, 'entity value incorrect');
}
