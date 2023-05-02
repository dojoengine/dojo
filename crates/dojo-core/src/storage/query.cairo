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

fn inner_id(state: felt252, keys: Span<felt252>, remain: usize) -> felt252 {
    gas::withdraw_gas_all(get_builtin_costs()).expect('Out of gas');

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
        Serde::<felt252>::serialize(ref serialized, input.computed_key);
        Serde::<Span<felt252>>::serialize(ref serialized, input.keys);
    }
    fn deserialize(ref serialized: Span::<felt252>) -> Option::<Query> {
        let address_domain = Serde::<u32>::deserialize(ref serialized)?;
        let partition = Serde::<felt252>::deserialize(ref serialized)?;
        let computed_key = Serde::<felt252>::deserialize(ref serialized)?;
        let mut arr = ArrayTrait::<felt252>::new();
        match Serde::<Span<felt252>>::deserialize(ref serialized) {
            Option::Some(keys) => {
                Option::Some(Query { address_domain: address_domain, partition: partition, keys: keys, computed_key: computed_key })
            },
            Option::None(_) => {
                Option::None(())
            },
        }
    }
}

impl ContractAddressIntoQuery of Into::<starknet::ContractAddress, Query> {
    fn into(self: starknet::ContractAddress) -> Query {
        QueryTrait::new_from_id(self.into())
    }
}

impl Felt252IntoQuery of Into::<felt252, Query> {
    fn into(self: felt252) -> Query {
        QueryTrait::new_from_id(self)

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
