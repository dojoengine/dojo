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
