use array::{ArrayTrait, SpanTrait};
use hash::LegacyHash;
use option::OptionTrait;
use serde::Serde;
use traits::Into;
use zeroable::IsZeroResult;
use starknet::ClassHashIntoFelt252;
use poseidon::poseidon_hash_span;

#[derive(Copy, Drop, Serde)]
struct Query {
    address_domain: u32,
    partition: felt252,
    keys: Span<felt252>,
}

trait QueryTrait {
    fn new(address_domain: u32, partition: felt252, keys: Span<felt252>) -> Query;
    fn new_from_id(id: felt252) -> Query;
    fn hash(self: @Query) -> felt252;
    fn table(self: @Query, component: felt252) -> felt252;
    fn keys(self: @Query) -> Span<felt252>;
}

impl QueryImpl of QueryTrait {
    fn new(address_domain: u32, partition: felt252, keys: Span<felt252>) -> Query {
        Query { address_domain, keys, partition }
    }

    fn new_from_id(id: felt252) -> Query {
        let mut keys = ArrayTrait::new();
        keys.append(id);
        QueryTrait::new(0, 0.into(), keys.span())
    }

    fn hash(self: @Query) -> felt252 {
        let keys = *self.keys;
        if keys.len() == 1 {
            return *keys.at(0);
        }

        let mut serialized = ArrayTrait::new();
        self.keys.serialize(ref serialized);
        poseidon_hash_span(serialized.span()).into()
    }

    fn table(self: @Query, component: felt252) -> felt252 {
        if *self.partition == 0.into() {
            return component.into();
        }

        let mut serialized = ArrayTrait::new();
        component.serialize(ref serialized);
        (*self.partition).serialize(ref serialized);
        let hash = poseidon_hash_span(serialized.span());
        hash.into()
    }

    fn keys(self: @Query) -> Span<felt252> {
        *self.keys
    }
}

impl LiteralIntoQuery<E0, impl E0Into: Into<E0, felt252>, impl E0Drop: Drop<E0>> of Into<E0, Query> {
    fn into(self: E0) -> Query {
        let mut keys = ArrayTrait::new();
        keys.append(E0Into::into(self));
        QueryTrait::new(0, 0.into(), keys.span())
    }
}

impl TupleSize1IntoQuery<
    E0, impl E0Into: Into<E0, felt252>, impl E0Drop: Drop<E0>
> of Into<(E0, ), Query> {
    fn into(self: (E0, )) -> Query {
        let (first) = self;
        let mut keys = ArrayTrait::new();
        keys.append(E0Into::into(first));
        QueryTrait::new(0, 0.into(), keys.span())
    }
}

impl TupleSize2IntoQuery<
    E0,
    E1,
    impl E0Into: Into<E0, felt252>,
    impl E0Drop: Drop<E0>,
    impl E1Into: Into<E1, felt252>,
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

impl TupleSize3IntoQuery<
    E0,
    E1,
    E2,
    impl E0Into: Into<E0, felt252>,
    impl E0Drop: Drop<E0>,
    impl E1Into: Into<E1, felt252>,
    impl E1Drop: Drop<E1>,
    impl E2Into: Into<E2, felt252>,
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

impl IntoPartitionedQuery<
    E0,
    E1,
    impl E0Into: Into<E0, felt252>,
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
