use array::ArrayTrait;
use array::SpanTrait;
use hash::LegacyHash;
use option::OptionTrait;
use serde::Serde;
use traits::Into;
use starknet::ClassHashIntoFelt252;
use dojo_core::serde::SpanSerde;

#[derive(Copy, Drop, Serde)]
struct Query {
    address_domain: u32,
    partition: felt252,
    keys: Span<felt252>,
    computed_key: felt252,
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
        if keys.len() == 1_usize {
            if partition == 0 {
                let computed_key = *keys.at(0_usize);
                return Query { address_domain, keys, partition, computed_key };
            }

            gas::withdraw_gas_all(get_builtin_costs()).expect('Out of gas');
            let computed_key = pedersen(partition, *keys.at(0_usize));
            return Query { address_domain, keys, partition, computed_key };
        }

        let computed_key = inner_id(0, keys, keys.len());
        Query { address_domain, keys, partition, computed_key }
    }
    fn new_from_id(id: felt252) -> Query {
        let mut keys = ArrayTrait::new();
        keys.append(id);
        QueryTrait::new(0, 0, keys.span())
    }
    fn id(self: @Query) -> felt252 {
        *self.computed_key
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
        self.computed_key
    }
}

fn inner_id(state: felt252, keys: Span<felt252>, remain: usize) -> felt252 {
    gas::withdraw_gas_all(get_builtin_costs()).expect('Out of gas');

    if (remain == 0_usize) {
        return state;
    }

    let next_state = pedersen(state, *keys.at(remain - 1_usize));
    return inner_id(next_state, keys, remain - 1_usize);
}

impl LegacyHashQuery of LegacyHash::<Query> {
    fn hash(state: felt252, value: Query) -> felt252 {
        LegacyHash::hash(state, value.into())
    }
}

impl LegacyHashClassHashQuery of LegacyHash::<(starknet::ClassHash, Query)> {
    fn hash(state: felt252, value: (starknet::ClassHash, Query)) -> felt252 {
        let (class_hash, query) = value;
        let class_hash_felt: felt252 = class_hash.into();
        let query_felt: felt252 = query.into();
        LegacyHash::hash(state, (class_hash_felt, query_felt))
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

impl TupleSize1IntoQuery<E0, impl E0Into: Into<E0, felt252>> of Into::<(E0, ), Query> {
    fn into(self: (E0, )) -> Query {
        let (first) = self;
        let mut keys = ArrayTrait::new();
        keys.append(E0Into::into(first));
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

impl TupleSize1IntoPartitionedQuery<E0, E1, impl E0Into: Into<E0, felt252>, impl E1Into: Into<E1, felt252>> of Into::<(E0, (E1, )), Query> {
    fn into(self: (E0, (E1, ))) -> Query {
        let (partition, keys) = self;
        let mut query: Query = keys.into();
        query.partition = E0Into::into(partition);
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
    assert(*query2.keys.at(0_usize) == 69, 'Incorrect query');
    assert(*query2.keys.at(1_usize) == 420, 'Incorrect query');
    let query3: Query = (69, 420, 777).into();
    assert(*query3.keys.at(0_usize) == 69, 'Incorrect query');
    assert(*query3.keys.at(1_usize) == 420, 'Incorrect query');
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
