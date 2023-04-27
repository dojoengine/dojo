use array::ArrayTrait;
use array::SpanTrait;
use hash::LegacyHash;
use option::OptionTrait;
use serde::Serde;
use traits::Into;
use zeroable::IsZeroResult;
use starknet::ClassHashIntoFelt252;
use poseidon::poseidon_hash_span;
use dojo_core::integer::u250;
use dojo_core::integer::Felt252IntoU250;
use dojo_core::integer::U250IntoFelt252;
use dojo_core::serde::SpanSerde;
use dojo_core::string::ShortString;

#[derive(Copy, Drop, Serde)]
struct Query {
    address_domain: u32,
    partition: u250,
    keys: Span<u250>,
    hash: u250,
}

trait QueryTrait {
    fn new(address_domain: u32, partition: u250, keys: Span<u250>) -> Query;
    fn new_from_id(id: u250) -> Query;
    fn id(self: @Query) -> u250;
    fn table(self: @Query, component: ShortString) -> u250;
    fn keys(self: @Query) -> Span<u250>;
}

impl QueryImpl of QueryTrait {
    fn new(address_domain: u32, partition: u250, keys: Span<u250>) -> Query {
        if keys.len() == 1_usize {
            if partition == 0.into() {
                let hash = *keys.at(0_usize);
                return Query { address_domain, keys, partition, hash };
            }

            gas::withdraw_gas_all(get_builtin_costs()).expect('Out of gas');

            let hash = LegacyHash::hash(0, (partition, *keys.at(0_usize)));
            return Query { address_domain, keys, partition, hash: hash.into() };
        }

        let mut serialized = ArrayTrait::new();
        Serde::serialize(ref serialized, partition);
        Serde::serialize(ref serialized, keys);
        let hash = poseidon_hash_span(serialized.span());
        Query { address_domain, keys, partition, hash: hash.into() }
    }
    fn new_from_id(id: u250) -> Query {
        let mut keys = ArrayTrait::new();
        keys.append(id);
        QueryTrait::new(0, 0.into(), keys.span())
    }
    fn id(self: @Query) -> u250 {
        *self.hash
    }
    fn table(self: @Query, component: ShortString) -> u250 {
        if *self.partition == 0.into() {
            return component.into();
        }

        let mut serialized = ArrayTrait::new();
        Serde::serialize(ref serialized, component);
        Serde::serialize(ref serialized, *self.partition);
        let hash = poseidon_hash_span(serialized.span());
        hash.into()
    }
    fn keys(self: @Query) -> Span<u250> {
        *self.keys
    }
}

impl QueryIntoFelt252 of Into::<Query, u250> {
    fn into(self: Query) -> u250 {
        self.hash
    }
}

impl LiteralIntoQuery<E0, impl E0Into: Into<E0, u250>, impl E0Drop: Drop<E0>> of Into::<E0, Query> {
    fn into(self: E0) -> Query {
        let mut keys = ArrayTrait::new();
        keys.append(E0Into::into(self));
        QueryTrait::new(0, 0.into(), keys.span())
    }
}

impl TupleSize1IntoQuery<E0, impl E0Into: Into<E0, u250>, impl E0Drop: Drop<E0>> of Into::<(E0,), Query> {
    fn into(self: (E0,)) -> Query {
        let (first) = self;
        let mut keys = ArrayTrait::new();
        keys.append(E0Into::into(first));
        QueryTrait::new(0, 0.into(), keys.span())
    }
}

impl TupleSize2IntoQuery<
        E0, E1,
        impl E0Into: Into<E0, u250>, impl E0Drop: Drop<E0>,
        impl E1Into: Into<E1, u250>, impl E1Drop: Drop<E1>,
    > of Into::<(E0, E1), Query> {
    fn into(self: (E0, E1)) -> Query {
        let (first, second) = self;
        let mut keys = ArrayTrait::new();
        keys.append(E0Into::into(first));
        keys.append(E1Into::into(second));
        QueryTrait::new(0, 0.into(), keys.span())
    }
}

impl TupleSize3IntoQuery<
        E0, E1, E2,
        impl E0Into: Into<E0, u250>, impl E0Drop: Drop<E0>,
        impl E1Into: Into<E1, u250>, impl E1Drop: Drop<E1>,
        impl E2Into: Into<E2, u250>, impl E2Drop: Drop<E2>,
    > of Into::<(E0, E1, E2), Query> {
    fn into(self: (E0, E1, E2)) -> Query {
        let (first, second, third) = self;
        let mut keys = ArrayTrait::new();
        keys.append(E0Into::into(first));
        keys.append(E1Into::into(second));
        keys.append(E2Into::into(third));
        QueryTrait::new(0, 0.into(), keys.span())
    }
}

trait IntoPartitioned<T, Query> {
    fn into_partitioned(self: T) -> Query;
}

impl IntoPartitionedQuery<
        E0, E1,
        impl E0Into: Into<E0, u250>, impl E0Drop: Drop<E0>,
        impl E1Into: Into<E1, Query>, impl E1Drop: Drop<E1>,
    > of IntoPartitioned::<(E0, E1), Query> {
    fn into_partitioned(self: (E0, E1)) -> Query {
        let (partition, keys) = self;
        let mut query: Query = E1Into::into(keys);
        query.partition = E0Into::into(partition);
        query
    }
}

#[test]
#[available_gas(2000000)]
fn test_query_id() {
    let mut keys = ArrayTrait::new();
    keys.append(420.into());
    let query: u250 = QueryTrait::new(0, 0.into(), keys.span()).into();
    assert(query == 420.into(), 'Incorrect hash');
}

#[test]
#[available_gas(2000000)]
fn test_query_into() {
    let query: Query = 420.into();
    assert(*query.keys.at(0_usize) == 420.into(), 'Incorrect query');
    let query1: Query = (69).into();
    assert(*query1.keys.at(0_usize) == 69.into(), 'Incorrect query');
    let query2: Query = (69, 420).into();
    assert(*query2.keys.at(0_usize) == 69.into(), 'Incorrect query');
    assert(*query2.keys.at(1_usize) == 420.into(), 'Incorrect query');
    let query3: Query = (69, 420, 777).into();
    assert(*query3.keys.at(0_usize) == 69.into(), 'Incorrect query');
    assert(*query3.keys.at(1_usize) == 420.into(), 'Incorrect query');
    assert(*query3.keys.at(2_usize) == 777.into(), 'Incorrect query');
}

#[test]
#[available_gas(2000000)]
fn test_partitioned_query_into() {
    let query: Query = (69, (420, )).into_partitioned();
    assert(query.partition == 69.into(), 'Incorrect partition');
    assert(*query.keys.at(0_usize) == 420.into(), 'Incorrect query');

    let query2: Query = (69, (420, 777)).into_partitioned();
    assert(query2.partition == 69.into(), 'Incorrect partition');
    assert(*query2.keys.at(1_usize) == 777.into(), 'Incorrect query');
}
