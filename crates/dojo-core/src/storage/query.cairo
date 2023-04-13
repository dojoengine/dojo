use array::ArrayTrait;
use array::SpanTrait;
use hash::LegacyHash;
use serde::Serde;
use traits::Into;
use starknet::ClassHashIntoFelt252;
use dojo_core::serde::SpanSerde;

#[derive(Drop)]
struct Query {
    address_domain: u32,
    partition: felt252,
    keys: Span<felt252>,
}

trait QueryTrait {
    fn new(address_domain: u32, partition: felt252, keys: Span<felt252>) -> Query;
    fn new_from_id(id: felt252) -> Query;
    fn id(self: @Query) -> felt252;
    fn table(self: @Query, component: felt252) -> felt252;
    fn keys(self: @Query) -> Span<felt252>;
}

impl QueryImpl of QueryTrait {
    fn new(address_domain: u32, partition: felt252, keys: Span<felt252>) -> Query {
        Query { address_domain, keys: keys, partition: partition }
    }
    fn new_from_id(id: felt252) -> Query {
        let mut keys = ArrayTrait::new();
        keys.append(id);
        Query { address_domain: 0_u32, keys: keys.span(), partition: 0 }
    }
    fn id(self: @Query) -> felt252 {
        inner_id(0, *self.keys, (*self.keys).len())
    }
    fn table(self: @Query, component: felt252) -> felt252 {
        if *self.partition == 0 {
            return component;
        }

        pedersen(component, *self.partition)
    }
    fn keys(self: @Query) -> Span<felt252> {
        *self.keys
    }
}

impl QueryIntoFelt252 of Into::<Query, felt252> {
    fn into(self: Query) -> felt252 {
        if self.keys.len() == 1_usize {
            return *self.keys.at(0_usize);
        }

        inner_id(0, self.keys, self.keys.len())
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

impl LegacyHashQuery of LegacyHash::<Query> {
    fn hash(state: felt252, query: Query) -> felt252 {
        LegacyHash::hash(state, query.into())
    }
}

impl LegacyHashClassHashQuery of LegacyHash::<(starknet::ClassHash, Query)> {
    fn hash(state: felt252, key: (starknet::ClassHash, Query)) -> felt252 {
        let (class_hash, query) = key;
        let class_hash_felt: felt252 = class_hash.into();
        let query_felt: felt252 = query.into();
        LegacyHash::hash(state, (class_hash_felt, query_felt))
    }
}

impl QuerySerde of serde::Serde::<Query> {
    fn serialize(ref serialized: Array::<felt252>, input: Query) {
        Serde::<u32>::serialize(ref serialized, input.address_domain);
        Serde::<felt252>::serialize(ref serialized, input.partition);
        Serde::<Span<felt252>>::serialize(ref serialized, input.keys);
    }
    fn deserialize(ref serialized: Span::<felt252>) -> Option::<Query> {
        let address_domain = Serde::<u32>::deserialize(ref serialized)?;
        let partition = Serde::<felt252>::deserialize(ref serialized)?;
        let mut arr = ArrayTrait::<felt252>::new();
        match Serde::<Span<felt252>>::deserialize(ref serialized) {
            Option::Some(keys) => {
                Option::Some(QueryTrait::new(address_domain, partition, keys))
            },
            Option::None(_) => {
                Option::None(())
            },
        }
    }
}

impl ContractAddressIntoQuery of Into::<starknet::ContractAddress, Query> {
    fn into(self: starknet::ContractAddress) -> Query {
        let mut keys = ArrayTrait::<felt252>::new();
        keys.append(self.into());
        QueryTrait::new(0, 0, keys.span())
    }
}

impl Felt252IntoQuery of Into::<felt252, Query> {
    fn into(self: felt252) -> Query {
        let mut keys = ArrayTrait::new();
        keys.append(self);
        QueryTrait::new(0, 0, keys.span())
    }
}

impl TupleSize1IntoQuery of Into::<(felt252, ), Query> {
    fn into(self: (felt252, )) -> Query {
        let (first) = self;
        let mut keys = ArrayTrait::new();
        keys.append(first);
        QueryTrait::new(0, 0, keys.span())
    }
}

impl TupleSize2IntoQuery of Into::<(felt252, felt252), Query> {
    fn into(self: (felt252, felt252)) -> Query {
        let (first, second) = self;
        let mut keys = ArrayTrait::new();
        keys.append(first);
        keys.append(second);
        QueryTrait::new(0, 0, keys.span())
    }
}

impl TupleSize3IntoQuery of Into::<(felt252, felt252, felt252), Query> {
    fn into(self: (felt252, felt252, felt252)) -> Query {
        let (first, second, third) = self;
        let mut keys = ArrayTrait::new();
        keys.append(first);
        keys.append(second);
        keys.append(third);
        QueryTrait::new(0, 0, keys.span())
    }
}

impl TupleSize1IntoPartitionedQuery of Into::<(felt252, (felt252, )), Query> {
    fn into(self: (felt252, (felt252, ))) -> Query {
        let (partition, keys) = self;
        let mut query: Query = keys.into();
        query.partition = partition;
        query
    }
}

impl TupleSize2IntoPartitionedQuery of Into::<(felt252, (felt252, felt252)), Query> {
    fn into(self: (felt252, (felt252, felt252))) -> Query {
        let (partition, keys) = self;
        let mut query: Query = keys.into();
        query.partition = partition;
        query
    }
}

#[test]
#[available_gas(2000000)]
fn test_query_id() {
    let mut keys = ArrayTrait::new();
    keys.append(420);
    let query = QueryTrait::new(0, 0, keys.span());
    assert(query.into() == 420, 'Incorrect hash');
}

#[test]
#[available_gas(2000000)]
fn test_query_into() {
    let query: Query = 420.into();
    assert(*query.keys.at(0_usize) == 420, 'Incorrect query');
    let query1: Query = (69).into();
    assert(*query1.keys.at(0_usize) == 69, 'Incorrect query');
    let query2: Query = (69, 420).into();
    // TODO: Figure out how to avoid the array copy error.
    // assert(*query2.keys.at(0_usize) == 69, 'Incorrect query');
    assert(*query2.keys.at(1_usize) == 420, 'Incorrect query');
    let query3: Query = (69, 420, 777).into();
    // assert(*query3.keys.at(0_usize) == 69, 'Incorrect query');
    // assert(*query3.keys.at(1_usize) == 420, 'Incorrect query');
    assert(*query3.keys.at(2_usize) == 777, 'Incorrect query');
}

#[test]
#[available_gas(2000000)]
fn test_partitioned_query_into() {
    let query: Query = (69, (420, )).into();
    assert(query.partition == 69, 'Incorrect partition');
    assert(*query.keys.at(0_usize) == 420, 'Incorrect query');

    let query2: Query = (69, (420, 777)).into();
    assert(query2.partition == 69, 'Incorrect partition');
    assert(*query2.keys.at(1_usize) == 777, 'Incorrect query');
}
