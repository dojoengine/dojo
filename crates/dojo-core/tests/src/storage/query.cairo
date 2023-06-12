use array::ArrayTrait;
use array::SpanTrait;
use hash::LegacyHash;
use option::OptionTrait;
use serde::Serde;
use traits::Into;
use zeroable::IsZeroResult;
use starknet::ClassHashIntoFelt252;
use poseidon::poseidon_hash_span;
use dojo_core::serde::SpanSerde;
use dojo_core::storage::key::{Column, Key, KeyTrait, ToKey, TupleSize2IntoKey, TupleSize3IntoKey};

#[test]
#[available_gas(2000000)]
fn test_key_id() {
    let mut columns = ArrayTrait::new();
    columns.append(420.into());
    let hash = KeyTrait::new(0, columns.span()).hash();
    assert(hash == 420.into(), 'Incorrect hash');
}

#[test]
#[available_gas(2000000)]
fn test_key_into() {
    let key: Key = 420.into();
    assert(*key.columns.at(0) == 420.into(), 'Incorrect key');
    let key1: Key = (69).into();
    assert(*key1.columns.at(0) == 69.into(), 'Incorrect key');
    let key2: Key = (69, 420).into();
    assert(*key2.columns.at(0) == 69.into(), 'Incorrect key');
    assert(*key2.columns.at(1) == 420.into(), 'Incorrect key');
    let key3: Key = (69, 420, 777).into();
    assert(*key3.columns.at(0) == 69.into(), 'Incorrect key');
    assert(*key3.columns.at(1) == 420.into(), 'Incorrect key');
    assert(*key3.columns.at(2) == 777.into(), 'Incorrect key');
}

#[test]
#[available_gas(2000000)]
fn test_indexed_key_into() {
    let key: Key = (69, 420).into();
    assert(*key.columns.at(0) == Column { value: 69, indexed: true }, 'Incorrect index');
    assert(*key.columns.at(0) == 420.into(), 'Incorrect key');

    let key2: Key = (69, 420, 777).into();
    assert(*key2.columns.at(0) == Column { value: 69, indexed: true }, 'Incorrect index');
    assert(*key2.columns.at(1) == 777.into(), 'Incorrect key');
}
