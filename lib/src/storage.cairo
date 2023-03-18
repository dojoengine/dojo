use array::ArrayTrait;
use array::SpanTrait;
use hash::LegacyHash;
use serde::Serde;
use debug::PrintTrait;

#[derive(Drop)]
struct StorageKey {
    keys: Array<felt252>,
    partition: PartitionKey,
}

#[derive(Drop, Copy)]
struct PartitionKey {
    component: felt252,
    partition: felt252,
}

trait PartitionKeyTrait {
    fn new(component: felt252, partition: felt252) -> PartitionKey;
    fn key(self: @PartitionKey) -> felt252;
}

impl PartitionKeyImpl of PartitionKeyTrait {
    fn new(component: felt252, partition: felt252) -> PartitionKey {
        PartitionKey { component: component, partition: partition,  }
    }

    fn key(self: @PartitionKey) -> felt252 {
        if *self.partition == 0 {
            return *self.component;
        }

        pedersen(*self.partition, *self.component)
    }
}

trait StorageKeyTrait {
    fn new(partition: PartitionKey, keys: Array<felt252>) -> StorageKey;
    fn id(self: @StorageKey) -> felt252;
}

impl StorageKeyImpl of StorageKeyTrait {
    fn new(partition: PartitionKey, keys: Array<felt252>) -> StorageKey {
        StorageKey { keys: keys, partition: partition,  }
    }

    fn id(self: @StorageKey) -> felt252 {
        if self.keys.len() == 1_usize {
            return *self.keys.at(0_usize);
        }

        inner_id(0, self.keys, self.keys.len())
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
    fn hash(state: felt252, key: StorageKey) -> felt252 {
        LegacyHash::hash(state, key.id())
    }
}

impl StorageKeySerde of serde::Serde::<StorageKey> {
    fn serialize(ref serialized: Array::<felt252>, input: StorageKey) {
        Serde::<PartitionKey>::serialize(ref serialized, input.partition);
        Serde::<Array<felt252>>::serialize(ref serialized, input.keys);
    }
    fn deserialize(ref serialized: Span::<felt252>) -> Option::<StorageKey> {
        let partition = Serde::<PartitionKey>::deserialize(ref serialized)?;
        let mut arr = ArrayTrait::<felt252>::new();
        match Serde::<Array<felt252>>::deserialize(ref serialized) {
            Option::Some(keys) => {
                Option::Some(StorageKey { partition: partition, keys: keys,  })
            },
            Option::None(_) => {
                Option::None(())
            },
        }
    }
}

impl LegacyHashPartitionKey of LegacyHash::<PartitionKey> {
    fn hash(state: felt252, partition: PartitionKey) -> felt252 {
        LegacyHash::hash(state, partition.key())
    }
}

impl PartitionKeySerde of serde::Serde::<PartitionKey> {
    fn serialize(ref serialized: Array::<felt252>, input: PartitionKey) {
        Serde::<felt252>::serialize(ref serialized, input.component);
        Serde::<felt252>::serialize(ref serialized, input.partition);
    }
    fn deserialize(ref serialized: Span::<felt252>) -> Option::<PartitionKey> {
        let component = *serialized.pop_front()?;
        let partition = *serialized.pop_front()?;
        Option::Some(PartitionKey { component: component, partition: partition,  })
    }
}

#[test]
#[available_gas(2000000)]
fn test_hash_key() {
    let mut keys = ArrayTrait::new();
    keys.append(420);
    let key = StorageKeyTrait::new(PartitionKeyTrait::new(0, 0), keys);
    assert(key.id() == 420, 'Incorrect hash');
}
