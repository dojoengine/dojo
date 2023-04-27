#[contract]
mod Index {
    use array::ArrayTrait;
    use array::SpanTrait;
    use traits::Into;
    use option::OptionTrait;

    use dojo_core::integer::u250;

    struct Storage {
        // Maps id to it's position in the table.
        // NOTE: ids is 1-indexed to allow for 0
        // to be used as a sentinel value.
        ids: LegacyMap::<(u250, u250), usize>,
        table_lens: LegacyMap::<u250, usize>,
        tables: LegacyMap::<(u250, usize), u250>,
    }

    fn create(table: u250, id: u250) {
        if exists(table, id) {
            return ();
        }

        let table_len = table_lens::read(table);
        ids::write((table, id), table_len + 1_usize);
        table_lens::write(table, table_len + 1_usize);
        tables::write((table, table_len), id);
    }

    fn delete(table: u250, id: u250) {
        if !exists(table, id) {
            return ();
        }

        let table_len = table_lens::read(table);
        let table_idx = ids::read((table, id)) - 1_usize;
        ids::write((table, id), 0_usize);
        table_lens::write(table, table_len - 1_usize);

        // Replace the deleted element with the last element.
        // NOTE: We leave the last element set as to not produce an unncessary state diff.
        tables::write((table, table_idx), tables::read((table, table_len - 1_usize)));
    }

    fn exists(table: u250, id: u250) -> bool {
        ids::read((table, id)) != 0_usize
    }

    fn query(table: u250) -> Array<u250> {
        let mut res = ArrayTrait::<u250>::new();
        let table_len = table_lens::read(table);
        _query(table, 0_usize, table_len, ref res);
        res
    }

    fn _query(table: u250, idx: usize, table_len: usize, ref res: Array<u250>) {
        gas::withdraw_gas_all(get_builtin_costs()).expect('Out of gas');

        if (idx == table_len) {
            return ();
        }

        res.append(tables::read((table, idx)));
        return _query(table, idx + 1_usize, table_len, ref res);
    }
}

mod tests {
    use array::ArrayTrait;
    use traits::Into;

    use dojo_core::integer::u250;
    use super::Index;

    #[test]
    #[available_gas(2000000)]
    fn test_index_entity() {
        let no_query = Index::query(69.into());
        assert(no_query.len() == 0_usize, 'entity indexed');

        Index::create(69.into(), 420.into());
        let query = Index::query(69.into());
        assert(query.len() == 1_usize, 'entity not indexed');
        assert(*query.at(0_usize) == 420.into(), 'entity value incorrect');

        Index::create(69.into(), 420.into());
        let noop_query = Index::query(69.into());
        assert(noop_query.len() == 1_usize, 'index should be noop');

        Index::create(69.into(), 1337.into());
        let two_query = Index::query(69.into());
        assert(two_query.len() == 2_usize, 'index should have two query');
        assert(*two_query.at(1_usize) == 1337.into(), 'entity value incorrect');
    }

    #[test]
    #[available_gas(2000000)]
    fn test_entity_delete_basic() {
        Index::create(69.into(), 420.into());
        let query = Index::query(69.into());
        assert(query.len() == 1_usize, 'entity not indexed');
        assert(*query.at(0_usize) == 420.into(), 'entity value incorrect');

        assert(Index::exists(69.into(), 420.into()), 'entity should exist');

        Index::delete(69.into(), 420.into());

        assert(!Index::exists(69.into(), 420.into()), 'entity should not exist');
        let no_query = Index::query(69.into());
        assert(no_query.len() == 0_usize, 'index should have no query');
    }

    #[test]
    #[available_gas(20000000)]
    fn test_entity_query_delete_shuffle() {
        let table = 1.into();
        Index::create(table, 10.into());
        Index::create(table, 20.into());
        Index::create(table, 30.into());
        assert(Index::query(table).len() == 3_usize, 'wrong size');

        Index::delete(table, 10.into());
        let entities = Index::query(table);
        assert(entities.len() == 2_usize, 'wrong size');
        assert(*entities.at(0_usize) == 30.into(), 'idx 0 not 30');
        assert(*entities.at(1_usize) == 20.into(), 'idx 1 not 20');
    }

    #[test]
    #[available_gas(20000000)]
    fn test_entity_query_delete_non_existing() {
        assert(Index::query(69.into()).len() == 0_usize, 'table len != 0');
        Index::delete(69.into(), 999.into()); // deleting non-existing should not panic
    }
}
