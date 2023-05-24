#[contract]
mod Index {
    use array::{ArrayTrait, SpanTrait};
    use traits::Into;
    use option::OptionTrait;

    use dojo_core::integer::u250;

    struct Storage {
        // Maps id to its position in the table.
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
        ids::write((table, id), table_len + 1);
        table_lens::write(table, table_len + 1);
        tables::write((table, table_len), id);
    }

    fn delete(table: u250, id: u250) {
        if !exists(table, id) {
            return ();
        }

        let table_len = table_lens::read(table);
        let table_idx = ids::read((table, id)) - 1;
        ids::write((table, id), 0);
        table_lens::write(table, table_len - 1);

        // Replace the deleted element with the last element.
        // NOTE: We leave the last element set as to not produce an unncessary state diff.
        tables::write((table, table_idx), tables::read((table, table_len - 1)));
    }

    fn exists(table: u250, id: u250) -> bool {
        ids::read((table, id)) != 0
    }

    fn query(table: u250) -> Array<u250> {
        let mut res = ArrayTrait::new();
        let table_len = table_lens::read(table);
        let mut idx: usize = 0;

        loop {
            if idx == table_len {
                break ();
            }
            
            res.append(tables::read((table, idx)));
            idx += 1;
        };

        res
    }
}
