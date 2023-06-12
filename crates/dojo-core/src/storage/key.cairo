use array::{ArrayTrait, SpanTrait};
use hash::LegacyHash;
use option::OptionTrait;
use serde::Serde;
use traits::Into;
use zeroable::IsZeroResult;
use starknet::ClassHashIntoFelt252;
use poseidon::poseidon_hash_span;
use dojo_core::serde::SpanSerde;

#[derive(Copy, Drop, PartialEq, Serde)]
struct Column {
    value: felt252,
    indexed: bool,
}

trait ToColumn<T, Column> {
    fn to_column(self: T) -> Column;
    fn indexed(self: T) -> Column;
}

impl TToColumn<T, impl TInto: Into<T, felt252>> of ToColumn<T, Column> {
    fn to_column(self: T) -> Column {
        let value = TInto::into(self);
        Column { value, indexed: false }
    }

    fn indexed(self: T) -> Column {
        let value = TInto::into(self);
        Column { value, indexed: true }
    }
}

impl TIntoColumn<T, impl TToColumn: ToColumn<T, Column>> of Into<T, Column> {
    fn into(self: T) -> Column {
        TToColumn::to_column(self)
    }
}

trait KeySchema {
    fn columns(self: @Key) -> Span<Column>;
}

#[derive(Copy, Drop, Serde)]
struct Key {
    address_domain: u32,
    columns: Span<Column>,
}

trait KeyTrait {
    fn new(address_domain: u32, columns: Span<Column>) -> Key;
    fn new_from_column(column: felt252) -> Key;
    fn hash(self: @Key) -> felt252;
    fn columns(self: @Key) -> Span<Column>;
}

impl KeyImpl of KeyTrait {
    fn new(address_domain: u32, columns: Span<Column>) -> Key {
        Key { address_domain, columns }
    }

    fn new_from_column(column: felt252) -> Key {
        let mut columns = ArrayTrait::new();
        columns.append(column.to_column());
        KeyTrait::new(0, columns.span())
    }

    fn hash(self: @Key) -> felt252 {
        let columns = *self.columns;
        if columns.len() == 1 {
            return *columns.at(0).value;
        }

        let mut serialized = ArrayTrait::new();
        self.columns.serialize(ref serialized);
        poseidon_hash_span(serialized.span())
    }

    fn columns(self: @Key) -> Span<Column> {
        *self.columns
    }
}

trait ToKey<T, Key> {
    fn to_key(self: T) -> Key;
}

impl TToKey<T, impl TToColumn: ToColumn<T, Column>, impl TDrop: Drop<T>> of ToKey<T, Key> {
    fn to_key(self: T) -> Key {
        let mut columns = ArrayTrait::new();
        columns.append(TToColumn::to_column(self));
        KeyTrait::new(0, columns.span())
    }
}

impl LiteralIntoKey<T, impl TToKey: ToKey<T, Key>, impl TDrop: Drop<T>> of Into<T, Key> {
    fn into(self: T) -> Key {
        TToKey::to_key(self)
    }
}

impl TupleSize1IntoKey<
    E0, impl E0Into: Into<E0, Column>, impl E0Drop: Drop<E0>
> of Into<(E0, ), Key> {
    fn into(self: (E0, )) -> Key {
        let (first) = self;
        let mut columns = ArrayTrait::new();
        columns.append(E0Into::into(first));
        KeyTrait::new(0, columns.span())
    }
}

impl TupleSize2IntoKey<
    E0,
    E1,
    impl E0Into: Into<E0, Column>,
    impl E0Drop: Drop<E0>,
    impl E1Into: Into<E1, Column>,
    impl E1Drop: Drop<E1>,
> of Into<(E0, E1), Key> {
    fn into(self: (E0, E1)) -> Key {
        let (first, second) = self;
        let mut columns = ArrayTrait::new();
        columns.append(E0Into::into(first));
        columns.append(E1Into::into(second));
        KeyTrait::new(0, columns.span())
    }
}

impl TupleSize3IntoKey<
    E0,
    E1,
    E2,
    impl E0Into: Into<E0, Column>,
    impl E0Drop: Drop<E0>,
    impl E1Into: Into<E1, Column>,
    impl E1Drop: Drop<E1>,
    impl E2Into: Into<E2, Column>,
    impl E2Drop: Drop<E2>,
> of Into<(E0, E1, E2), Key> {
    fn into(self: (E0, E1, E2)) -> Key {
        let (first, second, third) = self;
        let mut columns = ArrayTrait::new();
        columns.append(E0Into::into(first));
        columns.append(E1Into::into(second));
        columns.append(E2Into::into(third));
        KeyTrait::new(0, columns.span())
    }
}

impl TupleSize1IntoIndexedKey<
    E0,
    E1,
    impl E0Into: Into<E0, felt252>,
    impl E0Drop: Drop<E0>,
    impl E1Into: Into<E1, Column>,
    impl E1Drop: Drop<E1>
> of Into<((E0, ), (E1, )), Key> {
    fn into(self: ((E0, ), (E1, ))) -> Key {
        let ((first, ), (second, )) = self;
        let mut columns = ArrayTrait::new();
        columns.append(Column {
            value: E0Into::into(first),
            indexed: true,
        });    
        columns.append(E1Into::into(second));
        KeyTrait::new(0, columns.span())
    }
}

impl TupleSize2IntoIndexedKey<
    E0,
    E1,
    E2,
    impl E0Into: Into<E0, Column>,
    impl E0Drop: Drop<E0>,
    impl E1Into: Into<E1, Column>,
    impl E1Drop: Drop<E1>,
    impl E2Into: Into<E2, Column>,
    impl E2Drop: Drop<E2>,
> of Into<((E0, ), (E1, E2)), Key> {
    fn into(self: ((E0, ), (E1, E2))) -> Key {
        let ((first, ), (second, third)) = self;
        let mut columns = ArrayTrait::new();
        columns.append(E0Into::into(first));
        columns.append(E1Into::into(second));
        columns.append(E2Into::into(third));
        KeyTrait::new(0, columns.span())
    }
}
