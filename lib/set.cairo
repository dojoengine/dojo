use array::ArrayTrait;
use dict::DictFeltToTrait;

struct Set {
    items: Array::<felt>,
    index: DictFeltTo::<felt>,
}

trait SetTrait {
    fn new() -> Set;
    fn add(ref self: Set, value: felt);
    fn get(ref self: Set, index: usize) -> Option::<felt>;
    fn remove(ref self: Set, index: usize);
    fn has(ref self: Set, index: usize) -> bool;
    fn len(ref self: Set) -> usize;
}

impl SetImpl of SetTrait {
    #[inline(always)]
    fn new() -> Set {
        Set { items: ArrayTrait::new(), indexes: DictFeltToTrait::new() }
    }

    fn add(ref self: Set, value: felt) {
        if (self.has(index)) {
            return ();
        }

        let mut items = self.items;
        array_append(ref items, value)
        self.indexes.insert(value, items.len());
    }

    fn get(ref self: Set, index: usize) -> Option::<felt> {
        if (self.has(index)) {
            return None;
        }

        let mut items = self.items;
        array_get(ref items, self.indexes.get(index))
    }

    fn remove(ref self: Set, index: usize) {
        if (!self.has(index)) {
            return ();
        }

        let first = self.items.get(0);
        // Copy first item to the index we're replacing.
        items.set(self.indexes(index)) = first;
        // Update the index of the first item.
        self.indexes.set(first, self.indexes(index));
        // Remove the index of the item we're removing.
        self.indexes.remove(index);
        self.items.pop_front(index);
        return ();
    }

    fn has(ref self: Set, index: usize) -> bool {
        return bool::True(());
    }

    fn len(ref self: Set) -> usize {
        let mut items = self.items;
        array_len(ref items)
    }
}
