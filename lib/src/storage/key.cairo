use array::ArrayTrait;
use array::SpanTrait;
use hash::LegacyHash;
use serde::Serde;
use traits::Into;
use starknet::ClassHashIntoFelt252;
use dojo::serde::SpanSerde;

#[derive(Drop)]
struct StorageKey {
    partition: felt252,
    keys: Span<felt252>,
    computed_key: felt252,
}

trait StorageKeyTrait {
    fn new(partition: felt252, keys: Span<felt252>) -> StorageKey;
    fn new_from_id(id: felt252) -> StorageKey;
}

impl StorageKeyImpl of StorageKeyTrait {
    fn new(partition: felt252, keys: Span<felt252>) -> StorageKey {
        if keys.len() == 1_usize {
            if partition == 0 {
                let computed_key = *keys.at(0_usize);
                return StorageKey { keys: keys, partition: partition, computed_key: computed_key };
            }

            let computed_key = pedersen(partition, *keys.at(0_usize));
            return StorageKey { keys: keys, partition: partition, computed_key: computed_key };
        }

        let computed_key = inner_id(0, keys, keys.len());
        StorageKey { keys: keys, partition: partition, computed_key: computed_key }
    }

    fn new_from_id(id: felt252) -> StorageKey {
        let mut keys = ArrayTrait::new();
        keys.append(id);
        StorageKeyTrait::new(0, keys.span())
    }
}

impl StorageKeyIntoFelt252 of Into::<StorageKey, felt252> {
    fn into(self: StorageKey) -> felt252 {
        self.computed_key
    }
}

fn inner_id(state: felt252, keys: Span<felt252>, remain: usize) -> felt252 {
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
        LegacyHash::hash(state, key.into())
    }
}

impl LegacyHashClassHashStorageKey of LegacyHash::<(starknet::ClassHash, StorageKey)> {
    fn hash(state: felt252, key: (starknet::ClassHash, StorageKey)) -> felt252 {
        let (class_hash, storage_key) = key;
        let class_hash_felt: felt252 = class_hash.into();
        let storage_key_felt: felt252 = storage_key.into();
        LegacyHash::hash(state, (class_hash_felt, storage_key_felt))
    }
}

impl StorageKeySerde of serde::Serde::<StorageKey> {
    fn serialize(ref serialized: Array::<felt252>, input: StorageKey) {
        Serde::<felt252>::serialize(ref serialized, input.partition);
        Serde::<felt252>::serialize(ref serialized, input.computed_key);
        Serde::<Span<felt252>>::serialize(ref serialized, input.keys);
    }
    fn deserialize(ref serialized: Span::<felt252>) -> Option::<StorageKey> {
        let partition = Serde::<felt252>::deserialize(ref serialized)?;
        let computed_key = Serde::<felt252>::deserialize(ref serialized)?;
        let mut arr = ArrayTrait::<felt252>::new();
        match Serde::<Span<felt252>>::deserialize(ref serialized) {
            Option::Some(keys) => {
                Option::Some(
                    StorageKey { partition: partition, keys: keys, computed_key: computed_key }
                )
            },
            Option::None(_) => {
                Option::None(())
            },
        }
    }
}

impl ContractAddressIntoStorageKey of Into::<starknet::ContractAddress, StorageKey> {
    fn into(self: starknet::ContractAddress) -> StorageKey {
        let mut keys = ArrayTrait::<felt252>::new();
        keys.append(self.into());
        StorageKeyTrait::new(0, keys.span())
    }
}

impl Felt252IntoStorageKey of Into::<felt252, StorageKey> {
    fn into(self: felt252) -> StorageKey {
        let mut keys = ArrayTrait::new();
        keys.append(self);
        StorageKeyTrait::new(0, keys.span())
    }
}

impl TupleSize1IntoStorageKey of Into::<(felt252, ), StorageKey> {
    fn into(self: (felt252, )) -> StorageKey {
        let (first) = self;
        let mut keys = ArrayTrait::new();
        keys.append(first);
        StorageKeyTrait::new(0, keys.span())
    }
}

impl TupleSize2IntoStorageKey of Into::<(felt252, felt252), StorageKey> {
    fn into(self: (felt252, felt252)) -> StorageKey {
        let (first, second) = self;
        let mut keys = ArrayTrait::new();
        keys.append(first);
        keys.append(second);
        StorageKeyTrait::new(0, keys.span())
    }
}

impl TupleSize3IntoStorageKey of Into::<(felt252, felt252, felt252), StorageKey> {
    fn into(self: (felt252, felt252, felt252)) -> StorageKey {
        let (first, second, third) = self;
        let mut keys = ArrayTrait::new();
        keys.append(first);
        keys.append(second);
        keys.append(third);
        StorageKeyTrait::new(0, keys.span())
    }
}

impl TupleSize1IntoPartitionedStorageKey of Into::<(felt252, (felt252, )), StorageKey> {
    fn into(self: (felt252, (felt252, ))) -> StorageKey {
        let (partition, keys) = self;
        let mut storage_key: StorageKey = keys.into();
        storage_key.partition = partition;
        storage_key
    }
}

impl TupleSize2IntoPartitionedStorageKey of Into::<(felt252, (felt252, felt252)), StorageKey> {
    fn into(self: (felt252, (felt252, felt252))) -> StorageKey {
        let (partition, keys) = self;
        let mut storage_key: StorageKey = keys.into();
        storage_key.partition = partition;
        storage_key
    }
}

#[test]
#[available_gas(2000000)]
fn test_storagekey_id() {
    let mut keys = ArrayTrait::new();
    keys.append(420);
    let key = StorageKeyTrait::new(0, keys.span());
    assert(key.into() == 420, 'Incorrect hash');
}

#[test]
#[available_gas(2000000)]
fn test_storagekey_into() {
    let key: StorageKey = 420.into();
    assert(*key.keys.at(0_usize) == 420, 'Incorrect key');
    let key1: StorageKey = (69).into();
    assert(*key1.keys.at(0_usize) == 69, 'Incorrect key');
    let key2: StorageKey = (69, 420).into();
    // TODO: Figure out how to avoid the array copy error.
    // assert(*key2.keys.at(0_usize) == 69, 'Incorrect key');
    assert(*key2.keys.at(1_usize) == 420, 'Incorrect key');
    let key3: StorageKey = (69, 420, 777).into();
    // assert(*key3.keys.at(0_usize) == 69, 'Incorrect key');
    // assert(*key3.keys.at(1_usize) == 420, 'Incorrect key');
    assert(*key3.keys.at(2_usize) == 777, 'Incorrect key');
}

#[test]
#[available_gas(2000000)]
fn test_partitioned_storagekey_into() {
    let key: StorageKey = (69, (420, )).into();
    assert(key.partition == 69, 'Incorrect partition');
    assert(*key.keys.at(0_usize) == 420, 'Incorrect key');

    let key2: StorageKey = (69, (420, 777)).into();
    assert(key2.partition == 69, 'Incorrect partition');
    assert(*key2.keys.at(1_usize) == 777, 'Incorrect key');
}
