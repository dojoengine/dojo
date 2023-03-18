use array::ArrayTrait;
use array::SpanTrait;
use hash::LegacyHash;
use serde::Serde;

#[derive(Drop)]
struct StorageKey {
    component: felt252,
    partition: felt252,
    keys: Array<felt252>,
}

trait StorageKeyTrait {
    fn new(component: felt252, partition: felt252, keys: Array<felt252>) -> StorageKey;
    fn id(self: @StorageKey) -> felt252;
    fn partition(self: @StorageKey) -> felt252;
}

impl StorageKeyImpl of StorageKeyTrait {
    fn new(component: felt252, partition: felt252, keys: Array<felt252>) -> StorageKey {
        StorageKey {
            component: component,
            keys: keys,
            partition: partition,
        }
    }

    fn id(self: @StorageKey) -> felt252 {
        if self.keys.len() == 1_usize {
            return *self.keys.at(1_usize);
        }

        inner_id(0, self.keys, self.keys.len())
    }

    fn partition(self: @StorageKey) -> felt252 {
        if *self.partition == 0 {
            return *self.component;
        }

        pedersen(*self.component, *self.partition)
    }
}

fn inner_id(state: felt252, keys: @Array<felt252>, remain: usize) -> felt252 {
    match gas::withdraw_gas_all(get_builtin_costs()) {
        Option::Some(_) => {},
        Option::None(_) => {
            let mut data = ArrayTrait::new();
            data.append('OOG');
            panic(data);
        }
    }

    if (remain == 0_usize) {
        return state;
    }

    let next_state = pedersen(state, *keys.at(remain - 1_usize));
    return inner_id(next_state, keys, remain - 1_usize);
}

impl LegacyHashStorageKey of LegacyHash::<StorageKey> {
    fn hash(
        state: felt252, key: StorageKey
    ) -> felt252 {
        LegacyHash::hash(state, key.id())
    }
}

impl StorageKeySerde of serde::Serde::<StorageKey> {
    fn serialize(ref serialized: Array::<felt252>, input: StorageKey) {
        Serde::<felt252>::serialize(ref serialized, input.component);
        Serde::<felt252>::serialize(ref serialized, input.partition);
        Serde::<Array<felt252>>::serialize(ref serialized, input.keys);
    }
    fn deserialize(ref serialized: Span::<felt252>) -> Option::<StorageKey> {
        let component = *serialized.pop_front()?;
        let partition = *serialized.pop_front()?;
        let mut arr = ArrayTrait::<felt252>::new();
        match Serde::<Array<felt252>>::deserialize(ref serialized) {
            Option::Some(keys) => {
                Option::Some(StorageKey {
                    component: component,
                    partition: partition,
                    keys: keys,
                })
            },
            Option::None(_) => {
                Option::None(())
            },
        }
    }
}

#[test]
#[available_gas(2000000)]
fn test_hash_key() {
    let mut keys = ArrayTrait::new();
    keys.append(420);
    let key = StorageKeyTrait::new(0, 0, keys);
    assert(key.id() == 3326814640123998444291159895510695150223197938419765470484001946161999594643, 'Incorrect hash');
}
