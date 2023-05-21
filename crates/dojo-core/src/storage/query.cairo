use array::{ArrayTrait, SpanTrait};
use hash::LegacyHash;
use option::OptionTrait;
use serde::Serde;
use traits::Into;
use zeroable::IsZeroResult;
use starknet::ClassHashIntoFelt252;
use poseidon::poseidon_hash_span;
use dojo_core::{string::ShortString, serde::SpanSerde, integer::{u250, Felt252IntoU250, U250IntoFelt252}};

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
        if keys.len() == 1 {
            if partition == 0.into() {
                let hash = *keys.at(0);
                return Query { address_domain, keys, partition, hash };
            }

            gas::withdraw_gas_all(get_builtin_costs()).expect('Out of gas');

            let hash = LegacyHash::hash(0, (partition, *keys.at(0)));
            return Query { address_domain, keys, partition, hash: hash.into() };
        }

        let mut serialized = ArrayTrait::new();
        partition.serialize(ref serialized);
        keys.serialize(ref serialized);
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
        component.serialize(ref serialized);
        (*self.partition).serialize(ref serialized);
        let hash = poseidon_hash_span(serialized.span());
        hash.into()
    }
    fn keys(self: @Query) -> Span<u250> {
        *self.keys
    }
}

impl QueryIntoFelt252 of Into<Query, u250> {
    fn into(self: Query) -> u250 {
        self.hash
    }
}

impl LiteralIntoQuery<E0, impl E0Into: Into<E0, u250>, impl E0Drop: Drop<E0>> of Into<E0, Query> {
    fn into(self: E0) -> Query {
        let mut keys = ArrayTrait::new();
        keys.append(E0Into::into(self));
        QueryTrait::new(0, 0.into(), keys.span())
    }
}

impl TupleSize1IntoQuery<E0,
impl E0Into: Into<E0, u250>,
impl E0Drop: Drop<E0>> of Into<(E0, ), Query> {
    fn into(self: (E0, )) -> Query {
        let (first) = self;
        let mut keys = ArrayTrait::new();
        keys.append(E0Into::into(first));
        QueryTrait::new(0, 0.into(), keys.span())
    }
}

impl TupleSize2IntoQuery<E0,
E1,
impl E0Into: Into<E0, u250>,
impl E0Drop: Drop<E0>,
impl E1Into: Into<E1, u250>,
impl E1Drop: Drop<E1>,
> of Into<(E0, E1), Query> {
    fn into(self: (E0, E1)) -> Query {
        let (first, second) = self;
        let mut keys = ArrayTrait::new();
        keys.append(E0Into::into(first));
        keys.append(E1Into::into(second));
        QueryTrait::new(0, 0.into(), keys.span())
    }
}

impl TupleSize3IntoQuery<E0,
E1,
E2,
impl E0Into: Into<E0, u250>,
impl E0Drop: Drop<E0>,
impl E1Into: Into<E1, u250>,
impl E1Drop: Drop<E1>,
impl E2Into: Into<E2, u250>,
impl E2Drop: Drop<E2>,
> of Into<(E0, E1, E2), Query> {
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

impl IntoPartitionedQuery<E0,
E1,
impl E0Into: Into<E0, u250>,
impl E0Drop: Drop<E0>,
impl E1Into: Into<E1, Query>,
impl E1Drop: Drop<E1>,
> of IntoPartitioned<(E0, E1), Query> {
    fn into_partitioned(self: (E0, E1)) -> Query {
        let (partition, keys) = self;
        let mut query: Query = E1Into::into(keys);
        query.partition = E0Into::into(partition);
        query
    }
}