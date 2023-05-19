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
use dojo_core::storage::query::IntoPartitioned;
use dojo_core::storage::query::TupleSize2IntoQuery;
use dojo_core::storage::query::TupleSize3IntoQuery;
use dojo_core::storage::query::Query;
use dojo_core::storage::query::QueryTrait;

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
    assert(*query.keys.at(0) == 420.into(), 'Incorrect query');
    let query1: Query = (69).into();
    assert(*query1.keys.at(0) == 69.into(), 'Incorrect query');
    let query2: Query = (69, 420).into();
    assert(*query2.keys.at(0) == 69.into(), 'Incorrect query');
    assert(*query2.keys.at(1) == 420.into(), 'Incorrect query');
    let query3: Query = (69, 420, 777).into();
    assert(*query3.keys.at(0) == 69.into(), 'Incorrect query');
    assert(*query3.keys.at(1) == 420.into(), 'Incorrect query');
    assert(*query3.keys.at(2) == 777.into(), 'Incorrect query');
}

#[test]
#[available_gas(2000000)]
fn test_partitioned_query_into() {
    let query: Query = (69, (420, )).into_partitioned();
    assert(query.partition == 69.into(), 'Incorrect partition');
    assert(*query.keys.at(0) == 420.into(), 'Incorrect query');

    let query2: Query = (69, (420, 777)).into_partitioned();
    assert(query2.partition == 69.into(), 'Incorrect partition');
    assert(*query2.keys.at(1) == 777.into(), 'Incorrect query');
}
