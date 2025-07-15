use dojo::storage::dojo_store::DojoStore;

#[derive(Drop, Serde, Introspect, Default, Debug, PartialEq)]
enum E {
    #[default]
    A: u8,
    B: u32,
    C: (u8, (u16, u32), u64),
}

#[derive(Drop, Serde, Introspect, Debug, PartialEq, Default)]
struct S {
    x: u8,
    y: u32,
}


#[derive(Drop, Serde, Introspect, Default, Debug, PartialEq)]
struct SComplex {
    x: (u8, (u32, Array<u64>)),
    y: (S, S, Option<S>, u64),
}

#[derive(Drop, Serde, Introspect, Default, Debug, PartialEq)]
enum EComplex {
    A: (SComplex, Option<SComplex>),
    #[default]
    B,
}


#[derive(Drop, Introspect, Serde, Debug, PartialEq)]
struct GenericStruct<T> {
    value: T,
}

#[derive(Drop, Introspect, Serde, Default, Debug, PartialEq)]
enum GenericEnum<T> {
    #[default]
    A: T,
}

#[derive(Drop, Introspect, Serde, Debug, PartialEq)]
struct UseGenericStruct {
    x: GenericStruct<u16>,
    y: u8,
}

#[derive(Drop, Introspect, Serde, Debug, PartialEq)]
struct UseGenericEnum {
    x: GenericEnum<u16>,
    y: u8,
}

#[test]
fn test_dojo_store_primitives() {
    // felt252
    let mut serialized = array![];
    DojoStore::serialize(@1, ref serialized);
    assert_eq!(serialized, array![1], "DojoStore<felt252> serialization failed");

    let mut values = [1].span();
    let res = DojoStore::<felt252>::deserialize(ref values);
    assert_eq!(res, Option::Some(1), "DojoStore<felt252> deserialization failed");

    // bool
    let mut serialized = array![];
    DojoStore::serialize(@true, ref serialized);
    assert_eq!(serialized, array![1], "DojoStore<bool> serialization failed");

    let mut values = [1].span();
    let res = DojoStore::<bool>::deserialize(ref values);
    assert_eq!(res, Option::Some(true), "DojoStore<bool> deserialization failed");

    // u8
    let mut serialized = array![];
    DojoStore::serialize(@1_u8, ref serialized);
    assert_eq!(serialized, array![1], "DojoStore<u8> serialization failed");

    let mut values = [1].span();
    let res = DojoStore::<u8>::deserialize(ref values);
    assert_eq!(res, Option::Some(1_u8), "DojoStore<u8> deserialization failed");

    // u16
    let mut serialized = array![];
    DojoStore::serialize(@1_u16, ref serialized);
    assert_eq!(serialized, array![1], "DojoStore<u16> serialization failed");

    let mut values = [1].span();
    let res = DojoStore::<u16>::deserialize(ref values);
    assert_eq!(res, Option::Some(1_u16), "DojoStore<u16> deserialization failed");

    // u32
    let mut serialized = array![];
    DojoStore::serialize(@1_u32, ref serialized);
    assert_eq!(serialized, array![1], "DojoStore<u32> serialization failed");

    let mut values = [1].span();
    let res = DojoStore::<u32>::deserialize(ref values);
    assert_eq!(res, Option::Some(1_u32), "DojoStore<u32> deserialization failed");

    // u64
    let mut serialized = array![];
    DojoStore::serialize(@1_u64, ref serialized);
    assert_eq!(serialized, array![1], "DojoStore<u64> serialization failed");

    let mut values = [1].span();
    let res = DojoStore::<u64>::deserialize(ref values);
    assert_eq!(res, Option::Some(1_u64), "DojoStore<u64> deserialization failed");

    // u128
    let mut serialized = array![];
    DojoStore::serialize(@1_u128, ref serialized);
    assert_eq!(serialized, array![1], "DojoStore<u128> serialization failed");

    let mut values = [1].span();
    let res = DojoStore::<u128>::deserialize(ref values);
    assert_eq!(res, Option::Some(1_u128), "DojoStore<u128> deserialization failed");

    // u256
    let mut serialized = array![];
    DojoStore::serialize(@1_u256, ref serialized);
    assert_eq!(serialized, array![1, 0], "DojoStore<u256> serialization failed");

    let mut values = [1, 0].span();
    let res = DojoStore::<u256>::deserialize(ref values);
    assert_eq!(res, Option::Some(1_u256), "DojoStore<u256> deserialization failed");

    // i8
    let mut serialized = array![];
    DojoStore::serialize(@1_i8, ref serialized);
    assert_eq!(serialized, array![1], "DojoStore<i8> serialization failed");

    let mut values = [1].span();
    let res = DojoStore::<i8>::deserialize(ref values);
    assert_eq!(res, Option::Some(1_i8), "DojoStore<i8> deserialization failed");

    // i16
    let mut serialized = array![];
    DojoStore::serialize(@1_i16, ref serialized);
    assert_eq!(serialized, array![1], "DojoStore<i16> serialization failed");

    let mut values = [1].span();
    let res = DojoStore::<i16>::deserialize(ref values);
    assert_eq!(res, Option::Some(1_i16), "DojoStore<i16> deserialization failed");

    // i32
    let mut serialized = array![];
    DojoStore::serialize(@1_i32, ref serialized);
    assert_eq!(serialized, array![1], "DojoStore<i32> serialization failed");

    let mut values = [1].span();
    let res = DojoStore::<i32>::deserialize(ref values);
    assert_eq!(res, Option::Some(1_i32), "DojoStore<i32> deserialization failed");

    // i64
    let mut serialized = array![];
    DojoStore::serialize(@1_i64, ref serialized);
    assert_eq!(serialized, array![1], "DojoStore<i64> serialization failed");

    let mut values = [1].span();
    let res = DojoStore::<i64>::deserialize(ref values);
    assert_eq!(res, Option::Some(1_i64), "DojoStore<i64> deserialization failed");

    // i128
    let mut serialized = array![];
    DojoStore::serialize(@1_i128, ref serialized);
    assert_eq!(serialized, array![1], "DojoStore<i128> serialization failed");

    let mut values = [1].span();
    let res = DojoStore::<i128>::deserialize(ref values);
    assert_eq!(res, Option::Some(1_i128), "DojoStore<i128> deserialization failed");

    // ContractAddress
    let mut serialized = array![];
    let value: starknet::ContractAddress = 1.try_into().unwrap();

    DojoStore::serialize(@value, ref serialized);
    assert_eq!(serialized, array![1], "DojoStore<ContractAddress> serialization failed");

    let mut values = [1].span();
    let res = DojoStore::<starknet::ContractAddress>::deserialize(ref values);
    assert_eq!(res, Option::Some(value), "DojoStore<ContractAddress> deserialization failed");

    // ClassHash
    let mut serialized = array![];
    DojoStore::<starknet::ClassHash>::serialize(@1.try_into().unwrap(), ref serialized);
    assert_eq!(serialized, array![1], "DojoStore<ClassHash> serialization failed");

    let mut values = [1].span();
    let res = DojoStore::<starknet::ClassHash>::deserialize(ref values);
    assert_eq!(
        res, Option::Some(1.try_into().unwrap()), "DojoStore<ClassHash> deserialization failed",
    );

    // EthAddress
    let eth_address: starknet::EthAddress = 1.try_into().unwrap();

    let mut serialized = array![];
    DojoStore::serialize(@eth_address, ref serialized);
    assert_eq!(serialized, array![1], "DojoStore<EthAddress> serialization failed");

    let mut values = [1].span();
    let res = DojoStore::<starknet::EthAddress>::deserialize(ref values);
    assert_eq!(res, Option::Some(eth_address), "DojoStore<EthAddress> deserialization failed");

    // ByteArray
    let ba: ByteArray = "hello";

    let mut serialized = array![];
    DojoStore::serialize(@ba, ref serialized);
    assert_eq!(serialized, array![0, 0x68656c6c6f, 0x05], "DojoStore<i128> serialization failed");

    let mut values = [0, 0x68656c6c6f, 0x05].span();
    let res = DojoStore::<ByteArray>::deserialize(ref values);
    assert_eq!(res, Option::Some("hello"), "DojoStore<i128> deserialization failed");
}

#[test]
fn test_dojo_store_dynamic_arrays() {
    let arr: Array<u32> = array![1, 2, 3, 4];

    let mut serialized = array![];
    DojoStore::serialize(@arr, ref serialized);
    assert_eq!(serialized, array![4, 1, 2, 3, 4], "DojoStore<Array<u32>> serialization failed");

    let mut values = [4, 1, 2, 3, 4].span();
    let res = DojoStore::<Array<u32>>::deserialize(ref values);
    assert_eq!(
        res,
        Option::Some(array![1_u32, 2_u32, 3_u32, 4_u32]),
        "DojoStore<Array<u32>> deserialization failed",
    );

    let mut values = [].span();
    let res = DojoStore::<Array<u32>>::deserialize(ref values);
    assert_eq!(res, Option::None, "DojoStore<Array<u32>> deserialization failed");
}

#[test]
fn test_dojo_store_option() {
    let mut serialized = array![];
    DojoStore::serialize(@Option::Some(42_u32), ref serialized);
    assert_eq!(serialized, array![1, 42], "DojoStore<Option<u32>> serialization failed");

    let mut serialized = array![];
    DojoStore::serialize(@Option::<u32>::None, ref serialized);
    assert_eq!(serialized, array![2], "DojoStore<Option<u32>> serialization failed");

    let mut values = [1, 42].span();
    let res = DojoStore::<Option<u32>>::deserialize(ref values);
    assert_eq!(
        res, Option::Some(Option::Some(42_u32)), "DojoStore<Option<u32>> deserialization failed",
    );

    let mut values = [2].span();
    let res = DojoStore::<Option<u32>>::deserialize(ref values);
    assert_eq!(res, Option::Some(Option::None), "DojoStore<Option<u32>> deserialization failed");
}

#[test]
fn test_dojo_store_enums() {
    let e = E::B(42);

    let mut serialized = array![];
    DojoStore::serialize(@e, ref serialized);
    assert_eq!(serialized, array![2, 42], "DojoStore<E> serialization failed");

    let mut values = [2, 42].span();
    let res = DojoStore::<E>::deserialize(ref values);
    assert_eq!(res, Option::Some(E::B(42)), "DojoStore<E> deserialization failed");

    let mut values = [0].span();
    let res = DojoStore::<E>::deserialize(ref values);
    assert_eq!(res, Option::Some(E::A(0)), "DojoStore<E> deserialization failed");

    let mut values = [4].span();
    let res = DojoStore::<E>::deserialize(ref values);
    assert_eq!(res, Option::None, "DojoStore<E> deserialization failed");
}

#[test]
fn test_dojo_store_structs() {
    let s = S { x: 12, y: 42 };

    let mut serialized = array![];
    DojoStore::serialize(@s, ref serialized);
    assert_eq!(serialized, array![12, 42], "DojoStore<S> serialization failed");

    let mut values = [12, 42].span();
    let res = DojoStore::<S>::deserialize(ref values);
    assert_eq!(res, Option::Some(S { x: 12, y: 42 }), "DojoStore<S> deserialization failed");

    let mut values = [].span();
    let res = DojoStore::<S>::deserialize(ref values);
    assert_eq!(res, Option::None, "DojoStore<S> deserialization failed");
}

#[test]
fn test_dojo_store_tuples() {
    // use an enum to test tuple DojoStore
    let e = E::C((38, (12, 42), 98));

    let mut serialized = array![];
    DojoStore::serialize(@e, ref serialized);
    assert_eq!(serialized, array![3, 38, 12, 42, 98], "DojoStore<Tuple> serialization failed");

    let mut values = [3, 38, 12, 42, 98].span();
    let res = DojoStore::<E>::deserialize(ref values);
    assert_eq!(
        res, Option::Some(E::C((38, (12, 42), 98))), "DojoStore<Tuple> deserialization failed",
    );
}

#[test]
fn test_dojo_store_generic_struct() {
    let s = GenericStruct::<u32> { value: 1234567 };

    let mut serialized = array![];
    DojoStore::serialize(@s, ref serialized);
    assert_eq!(serialized, array![1234567], "DojoStore<GenericStruct<u32>> serialization failed");

    let mut values = [1234567].span();
    let res = DojoStore::<GenericStruct<u32>>::deserialize(ref values);
    assert_eq!(
        res,
        Option::Some(GenericStruct::<u32> { value: 1234567 }),
        "DojoStore<GenericStruct<u32>> deserialization failed",
    );
}

#[test]
fn test_dojo_store_generic_enum() {
    let e = GenericEnum::<u32>::A(1234567);

    let mut serialized = array![];
    DojoStore::serialize(@e, ref serialized);
    assert_eq!(serialized, array![1, 1234567], "DojoStore<GenericEnum<u32>> serialization failed");

    let mut values = [1, 1234567].span();
    let res = DojoStore::<GenericEnum<u32>>::deserialize(ref values);
    assert_eq!(
        res,
        Option::Some(GenericEnum::<u32>::A(1234567)),
        "DojoStore<GenericEnum<u32>> deserialization failed",
    );
}


#[test]
fn test_dojo_store_use_generic_struct() {
    let s = UseGenericStruct { x: GenericStruct { value: 12345 }, y: 42 };

    let mut serialized = array![];
    DojoStore::serialize(@s, ref serialized);
    assert_eq!(serialized, array![12345, 42], "DojoStore<UseGenericStruct> serialization failed");

    let mut values = [12345, 42].span();
    let res = DojoStore::<UseGenericStruct>::deserialize(ref values);
    assert_eq!(
        res,
        Option::Some(UseGenericStruct { x: GenericStruct { value: 12345 }, y: 42 }),
        "DojoStore<UseGenericStruct> deserialization failed",
    );
}

#[test]
fn test_dojo_store_use_generic_enum() {
    let e = UseGenericEnum { x: GenericEnum::A(12345), y: 42 };

    let mut serialized = array![];
    DojoStore::serialize(@e, ref serialized);
    assert_eq!(serialized, array![1, 12345, 42], "DojoStore<UseGenericEnum> serialization failed");

    let mut values = [1, 12345, 42].span();
    let res = DojoStore::<UseGenericEnum>::deserialize(ref values);
    assert_eq!(
        res,
        Option::Some(UseGenericEnum { x: GenericEnum::A(12345), y: 42 }),
        "DojoStore<UseGenericEnum> deserialization failed",
    );
}

#[test]
fn test_mix() {
    let e = EComplex::A(
        (
            SComplex {
                x: (2_u8, (42_u32, array![1_u64, 2_u64, 3_u64])),
                y: (S { x: 1, y: 2 }, S { x: 6, y: 7 }, Option::Some(S { x: 67, y: 456 }), 987_u64),
            },
            Option::Some(
                SComplex {
                    x: (12_u8, (78_u32, array![6_u64, 9_u64, 34_u64])),
                    y: (S { x: 10, y: 20 }, S { x: 60, y: 70 }, Option::<S>::None, 578_u64),
                },
            ),
        ),
    );

    let values = array![
        1, 2, 42, 3, 1, 2, 3, 1, 2, 6, 7, 1, 67, 456, 987, 1, 12, 78, 3, 6, 9, 34, 10, 20, 60, 70,
        2, 578,
    ];
    let mut serialized = array![];
    DojoStore::serialize(@e, ref serialized);
    assert_eq!(serialized, values.clone(), "DojoStore<EComplex> serialization failed");

    let mut values = values.span();
    let res = DojoStore::<EComplex>::deserialize(ref values);
    assert_eq!(res, Option::Some(e), "DojoStore<EComplex> deserialization failed");
}
