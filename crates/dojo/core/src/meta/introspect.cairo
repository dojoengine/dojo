use core::panics::panic_with_byte_array;
/// This file contains the implementation of the `Introspect` trait.
///
/// The introspection is used to get the size and layout of a type.
/// It is important to note that signed integers in Cairo are always using `252` bits.

use dojo::meta::Layout;
use dojo::storage::packing;
use dojo::utils::sum_sizes;

// Each index matches with a primitive types in both arrays (main and nested).
// The main array represents the source primitive while nested arrays represents
// destination primitives.
// 'bool': 0
// 'u8': 1
// 'u16': 2
// 'u32': 3
// 'u64': 4
// 'u128': 5
// 'u256': 6
// 'i8': 7
// 'i16': 8
// 'i32': 9
// 'i64': 10
// 'i128': 11
// 'felt252': 12
// 'ClassHash': 13
// 'ContractAddress': 14
// 'EthAddress': 15
const ALLOWED_PRIMITIVE_UPGRADES: [[bool; 16]; 16] = [
    // bool
    [
        true, false, false, false, false, false, false, false, false, false, false, false, true,
        false, false, false,
    ],
    // u8
    [
        false, true, true, true, true, true, false, false, false, false, false, false, true, false,
        false, false,
    ],
    // u16
    [
        false, false, true, true, true, true, false, false, false, false, false, false, true, false,
        false, false,
    ],
    // u32
    [
        false, false, false, true, true, true, false, false, false, false, false, false, true,
        false, false, false,
    ],
    // u64
    [
        false, false, false, false, true, true, false, false, false, false, false, false, true,
        false, false, false,
    ],
    // u128
    [
        false, false, false, false, false, true, false, false, false, false, false, false, true,
        false, false, false,
    ],
    // u256
    [
        false, false, false, false, false, false, true, false, false, false, false, false, false,
        false, false, false,
    ],
    // i8
    [
        false, false, false, false, false, false, false, true, true, true, true, true, true, false,
        false, false,
    ],
    // i16
    [
        false, false, false, false, false, false, false, false, true, true, true, true, true, false,
        false, false,
    ],
    // i32
    [
        false, false, false, false, false, false, false, false, false, true, true, true, true,
        false, false, false,
    ],
    // i64
    [
        false, false, false, false, false, false, false, false, false, false, true, true, true,
        false, false, false,
    ],
    // i128
    [
        false, false, false, false, false, false, false, false, false, false, false, true, true,
        false, false, false,
    ],
    // felt252
    [
        false, false, false, false, false, false, false, false, false, false, false, false, true,
        true, true, false,
    ],
    // ClassHash
    [
        false, false, false, false, false, false, false, false, false, false, false, false, true,
        true, true, false,
    ],
    // ContractAddress
    [
        false, false, false, false, false, false, false, false, false, false, false, false, true,
        true, true, false,
    ],
    // EthAddress
    [
        false, false, false, false, false, false, false, false, false, false, false, false, true,
        true, true, true,
    ],
];

#[inline(always)]
fn primitive_to_index(primitive: felt252) -> u32 {
    if primitive == 'bool' {
        return 0;
    }
    if primitive == 'u8' {
        return 1;
    }
    if primitive == 'u16' {
        return 2;
    }
    if primitive == 'u32' {
        return 3;
    }
    if primitive == 'u64' {
        return 4;
    }
    if primitive == 'u128' {
        return 5;
    }
    if primitive == 'u256' {
        return 6;
    }
    if primitive == 'i8' {
        return 7;
    }
    if primitive == 'i16' {
        return 8;
    }
    if primitive == 'i32' {
        return 9;
    }
    if primitive == 'i64' {
        return 10;
    }
    if primitive == 'i128' {
        return 11;
    }
    if primitive == 'felt252' {
        return 12;
    }
    if primitive == 'ClassHash' || primitive == 'starknet::Classhash' {
        return 13;
    }
    if primitive == 'ContractAddress' || primitive == 'starknet::ContractAddress' {
        return 14;
    }
    if primitive == 'EthAddress' {
        return 15;
    }

    if primitive == 'usize' {
        panic_with_byte_array(
            @format!("Prefer using u32 instead of usize as usize size is architecture-dependent."),
        )
    }

    panic_with_byte_array(
        @format!("The introspection of the primitive type {primitive} is not supported."),
    )
}

/// Note that for `Array` and `FixedArray` we can't directly use `Ty` as it will cause infinite
/// recursion, so we decided to use a Span with one item only.
/// Note also that, now, Torii uses this `Span` for specific processing on its side, so it cannot be
/// changed directly by a Box<Ty>.
#[derive(Copy, Drop, Serde, Debug, PartialEq)]
pub enum Ty {
    Primitive: felt252,
    Struct: Struct,
    Enum: Enum,
    Tuple: Span<Ty>,
    Array: Span<Ty>,
    ByteArray,
    FixedArray: (Span<Ty>, u32),
}

#[derive(Copy, Drop, Serde, Debug, PartialEq)]
pub struct Struct {
    pub name: felt252,
    pub attrs: Span<felt252>,
    pub children: Span<Member>,
}

#[derive(Copy, Drop, Serde, Debug, PartialEq)]
pub struct Enum {
    pub name: felt252,
    pub attrs: Span<felt252>,
    pub children: Span<(felt252, Ty)>,
}

#[derive(Copy, Drop, Serde, Debug, PartialEq)]
pub struct Member {
    pub name: felt252,
    pub attrs: Span<felt252>,
    pub ty: Ty,
}

pub trait TyCompareTrait<T> {
    fn is_a_key_upgrade_of(self: @T, old: @T) -> bool;
    fn is_an_upgrade_of(self: @T, old: @T) -> bool;
}

impl PrimitiveCompareImpl of TyCompareTrait<felt252> {
    fn is_a_key_upgrade_of(self: @felt252, old: @felt252) -> bool {
        Self::is_an_upgrade_of(self, old)
    }

    fn is_an_upgrade_of(self: @felt252, old: @felt252) -> bool {
        if self == old {
            return true;
        }

        let new_index = primitive_to_index(*self);
        let old_index = primitive_to_index(*old);

        let allowed_upgrades = ALLOWED_PRIMITIVE_UPGRADES.span();
        let allowed_upgrades = allowed_upgrades[old_index].span();
        *allowed_upgrades[new_index]
    }
}

impl TyCompareImpl of TyCompareTrait<Ty> {
    fn is_a_key_upgrade_of(self: @Ty, old: @Ty) -> bool {
        match (self, old) {
            (Ty::Primitive(n), Ty::Primitive(o)) => n.is_a_key_upgrade_of(o),
            (Ty::Struct(n), Ty::Struct(o)) => n.is_a_key_upgrade_of(o),
            (Ty::Enum(n), Ty::Enum(o)) => n.is_a_key_upgrade_of(o),
            (Ty::Array(n), Ty::Array(o)) => { (*n).at(0).is_a_key_upgrade_of((*o).at(0)) },
            (
                Ty::FixedArray(n), Ty::FixedArray(o),
            ) => {
                let (n_ty, n_len) = n.at(0);
                let (o_ty, o_len) = o.at(0);

                n_ty.is_a_key_upgrade_of(o_ty) && n_len == o_len
            },
            (
                Ty::Tuple(n), Ty::Tuple(o),
            ) => {
                let n = *n;
                let o = *o;

                if n.len() != o.len() {
                    return false;
                }

                for i in 0..n.len() {
                    if !n.at(i).is_a_key_upgrade_of(o.at(i)) {
                        return false;
                    }
                }

                true
            },
            (Ty::ByteArray, Ty::ByteArray) => true,
            _ => false,
        }
    }

    fn is_an_upgrade_of(self: @Ty, old: @Ty) -> bool {
        match (self, old) {
            (Ty::Primitive(n), Ty::Primitive(o)) => n.is_an_upgrade_of(o),
            (Ty::Struct(n), Ty::Struct(o)) => n.is_an_upgrade_of(o),
            (Ty::Array(n), Ty::Array(o)) => { (*n).at(0).is_an_upgrade_of((*o).at(0)) },
            (
                Ty::FixedArray(n), Ty::FixedArray(o),
            ) => {
                let (n_ty, n_len) = n;
                let n_ty = n_ty.at(0);

                let (o_ty, o_len) = o;
                let o_ty = o_ty.at(0);

                n_ty.is_an_upgrade_of(o_ty) && n_len >= o_len
            },
            (
                Ty::Tuple(n), Ty::Tuple(o),
            ) => {
                let n = *n;
                let o = *o;

                if n.len() < o.len() {
                    return false;
                }

                for i in 0..o.len() {
                    if !n.at(i).is_an_upgrade_of(o.at(i)) {
                        return false;
                    }
                }

                true
            },
            (Ty::ByteArray, Ty::ByteArray) => true,
            (Ty::Enum(n), Ty::Enum(o)) => n.is_an_upgrade_of(o),
            _ => false,
        }
    }
}

impl EnumCompareImpl of TyCompareTrait<Enum> {
    fn is_a_key_upgrade_of(self: @Enum, old: @Enum) -> bool {
        if self == old {
            return true;
        }

        let n = *self;
        let o = *old;

        if n.name != o.name || n.attrs != o.attrs || n.children.len() < o.children.len() {
            return false;
        }

        // new variants are allowed and existing variants must follow key upgrade rules.
        for i in 0..o.children.len() {
            let (new_name, new_ty) = n.children[i];
            let (old_name, old_ty) = o.children[i];

            if new_name != old_name || !new_ty.is_a_key_upgrade_of(old_ty) {
                return false;
            }
        }

        true
    }

    fn is_an_upgrade_of(self: @Enum, old: @Enum) -> bool {
        if self.name != old.name
            || self.attrs != old.attrs
            || (*self.children).len() < (*old.children).len() {
            return false;
        }

        for i in 0..(*old.children).len() {
            let (old_name, old_ty) = *old.children[i];
            let (new_name, new_ty) = *self.children[i];

            // renaming is not allowed as checking if variants have not been reordered
            // could be quite challenging
            if new_name != old_name {
                return false;
            }

            if !new_ty.is_an_upgrade_of(@old_ty) {
                return false;
            }
        }

        true
    }
}

impl StructCompareImpl of TyCompareTrait<Struct> {
    fn is_a_key_upgrade_of(self: @Struct, old: @Struct) -> bool {
        if self.name != old.name
            || self.attrs != old.attrs
            || (*self.children).len() != (*old.children).len() {
            return false;
        }

        for i in 0..(*old.children).len() {
            if !self.children[i].is_a_key_upgrade_of(old.children[i]) {
                return false;
            }
        }

        true
    }

    fn is_an_upgrade_of(self: @Struct, old: @Struct) -> bool {
        if self.name != old.name
            || self.attrs != old.attrs
            || (*self.children).len() < (*old.children).len() {
            return false;
        }

        for i in 0..(*old.children).len() {
            if !self.children[i].is_an_upgrade_of(old.children[i]) {
                return false;
            }
        }

        true
    }
}

impl MemberCompareImpl of TyCompareTrait<Member> {
    fn is_a_key_upgrade_of(self: @Member, old: @Member) -> bool {
        if self.name != old.name || self.attrs != old.attrs {
            return false;
        }

        self.ty.is_a_key_upgrade_of(old.ty)
    }

    fn is_an_upgrade_of(self: @Member, old: @Member) -> bool {
        if self.name != old.name || self.attrs != old.attrs {
            return false;
        }

        let mut is_key = false;

        for attr in self.attrs {
            if *attr == 'key' {
                is_key = true;
                break;
            }
        }

        if is_key {
            self.ty.is_a_key_upgrade_of(old.ty)
        } else {
            self.ty.is_an_upgrade_of(old.ty)
        }
    }
}

pub trait Introspect<T> {
    fn size() -> Option<usize>;
    fn layout() -> Layout;
    fn ty() -> Ty;
}

pub impl Introspect_felt252 of Introspect<felt252> {
    #[inline(always)]
    fn size() -> Option<usize> {
        Option::Some(1)
    }
    #[inline(always)]
    fn layout() -> Layout {
        Layout::Fixed([packing::PACKING_MAX_BITS].span())
    }
    #[inline(always)]
    fn ty() -> Ty {
        Ty::Primitive('felt252')
    }
}

pub impl Introspect_bytes31 of Introspect<bytes31> {
    #[inline(always)]
    fn size() -> Option<usize> {
        Option::Some(1)
    }
    #[inline(always)]
    fn layout() -> Layout {
        Layout::Fixed([248].span())
    }
    #[inline(always)]
    fn ty() -> Ty {
        Ty::Primitive('bytes31')
    }
}

pub impl Introspect_bool of Introspect<bool> {
    #[inline(always)]
    fn size() -> Option<usize> {
        Option::Some(1)
    }
    #[inline(always)]
    fn layout() -> Layout {
        Layout::Fixed([1].span())
    }
    #[inline(always)]
    fn ty() -> Ty {
        Ty::Primitive('bool')
    }
}

pub impl Introspect_u8 of Introspect<u8> {
    #[inline(always)]
    fn size() -> Option<usize> {
        Option::Some(1)
    }
    #[inline(always)]
    fn layout() -> Layout {
        Layout::Fixed([8].span())
    }
    #[inline(always)]
    fn ty() -> Ty {
        Ty::Primitive('u8')
    }
}

pub impl Introspect_u16 of Introspect<u16> {
    #[inline(always)]
    fn size() -> Option<usize> {
        Option::Some(1)
    }
    #[inline(always)]
    fn layout() -> Layout {
        Layout::Fixed([16].span())
    }
    #[inline(always)]
    fn ty() -> Ty {
        Ty::Primitive('u16')
    }
}

pub impl Introspect_u32 of Introspect<u32> {
    #[inline(always)]
    fn size() -> Option<usize> {
        Option::Some(1)
    }
    #[inline(always)]
    fn layout() -> Layout {
        Layout::Fixed([32].span())
    }
    #[inline(always)]
    fn ty() -> Ty {
        Ty::Primitive('u32')
    }
}

pub impl Introspect_u64 of Introspect<u64> {
    #[inline(always)]
    fn size() -> Option<usize> {
        Option::Some(1)
    }
    #[inline(always)]
    fn layout() -> Layout {
        Layout::Fixed([64].span())
    }
    #[inline(always)]
    fn ty() -> Ty {
        Ty::Primitive('u64')
    }
}

pub impl Introspect_u128 of Introspect<u128> {
    #[inline(always)]
    fn size() -> Option<usize> {
        Option::Some(1)
    }
    #[inline(always)]
    fn layout() -> Layout {
        Layout::Fixed([128].span())
    }
    #[inline(always)]
    fn ty() -> Ty {
        Ty::Primitive('u128')
    }
}

pub impl Introspect_u256 of Introspect<u256> {
    #[inline(always)]
    fn size() -> Option<usize> {
        Option::Some(2)
    }
    #[inline(always)]
    fn layout() -> Layout {
        Layout::Fixed([128, 128].span())
    }
    #[inline(always)]
    fn ty() -> Ty {
        Ty::Primitive('u256')
    }
}

pub impl Introspect_i8 of Introspect<i8> {
    #[inline(always)]
    fn size() -> Option<usize> {
        Option::Some(1)
    }
    #[inline(always)]
    fn layout() -> Layout {
        Layout::Fixed([packing::PACKING_MAX_BITS].span())
    }
    #[inline(always)]
    fn ty() -> Ty {
        Ty::Primitive('i8')
    }
}

pub impl Introspect_i16 of Introspect<i16> {
    #[inline(always)]
    fn size() -> Option<usize> {
        Option::Some(1)
    }
    #[inline(always)]
    fn layout() -> Layout {
        Layout::Fixed([packing::PACKING_MAX_BITS].span())
    }
    #[inline(always)]
    fn ty() -> Ty {
        Ty::Primitive('i16')
    }
}

pub impl Introspect_i32 of Introspect<i32> {
    #[inline(always)]
    fn size() -> Option<usize> {
        Option::Some(1)
    }
    #[inline(always)]
    fn layout() -> Layout {
        Layout::Fixed([packing::PACKING_MAX_BITS].span())
    }
    #[inline(always)]
    fn ty() -> Ty {
        Ty::Primitive('i32')
    }
}

pub impl Introspect_i64 of Introspect<i64> {
    #[inline(always)]
    fn size() -> Option<usize> {
        Option::Some(1)
    }
    #[inline(always)]
    fn layout() -> Layout {
        Layout::Fixed([packing::PACKING_MAX_BITS].span())
    }
    #[inline(always)]
    fn ty() -> Ty {
        Ty::Primitive('i64')
    }
}

pub impl Introspect_i128 of Introspect<i128> {
    #[inline(always)]
    fn size() -> Option<usize> {
        Option::Some(1)
    }
    #[inline(always)]
    fn layout() -> Layout {
        Layout::Fixed([packing::PACKING_MAX_BITS].span())
    }
    #[inline(always)]
    fn ty() -> Ty {
        Ty::Primitive('i128')
    }
}

pub impl Introspect_address of Introspect<starknet::ContractAddress> {
    #[inline(always)]
    fn size() -> Option<usize> {
        Option::Some(1)
    }
    #[inline(always)]
    fn layout() -> Layout {
        Layout::Fixed([packing::PACKING_MAX_BITS].span())
    }
    #[inline(always)]
    fn ty() -> Ty {
        Ty::Primitive('ContractAddress')
    }
}

pub impl Introspect_classhash of Introspect<starknet::ClassHash> {
    #[inline(always)]
    fn size() -> Option<usize> {
        Option::Some(1)
    }
    #[inline(always)]
    fn layout() -> Layout {
        Layout::Fixed([packing::PACKING_MAX_BITS].span())
    }
    #[inline(always)]
    fn ty() -> Ty {
        Ty::Primitive('ClassHash')
    }
}

pub impl Introspect_ethaddress of Introspect<starknet::EthAddress> {
    #[inline(always)]
    fn size() -> Option<usize> {
        Option::Some(1)
    }
    #[inline(always)]
    fn layout() -> Layout {
        Layout::Fixed([packing::PACKING_MAX_BITS].span())
    }
    #[inline(always)]
    fn ty() -> Ty {
        Ty::Primitive('EthAddress')
    }
}

pub impl Introspect_bytearray of Introspect<ByteArray> {
    #[inline(always)]
    fn size() -> Option<usize> {
        Option::None
    }
    #[inline(always)]
    fn layout() -> Layout {
        Layout::ByteArray
    }
    #[inline(always)]
    fn ty() -> Ty {
        Ty::ByteArray
    }
}

pub impl Introspect_option<T, +Introspect<T>> of Introspect<Option<T>> {
    #[inline(always)]
    fn size() -> Option<usize> {
        Option::None
    }

    #[inline(always)]
    fn layout() -> Layout {
        Layout::Enum(
            [
                dojo::meta::FieldLayout { // Some
                selector: 1, layout: Introspect::<T>::layout() },
                dojo::meta::FieldLayout { // None
                selector: 2, layout: Layout::Fixed([].span()) },
            ]
                .span(),
        )
    }

    #[inline(always)]
    fn ty() -> Ty {
        Ty::Enum(
            Enum {
                name: 'Option<T>',
                attrs: [].span(),
                children: [('Some(T)', Introspect::<T>::ty()), ('None', Ty::Tuple([].span()))]
                    .span(),
            },
        )
    }
}

pub impl Introspect_array<T, +Introspect<T>> of Introspect<Array<T>> {
    #[inline(always)]
    fn size() -> Option<usize> {
        Option::None
    }
    #[inline(always)]
    fn layout() -> Layout {
        Layout::Array([Introspect::<T>::layout()].span())
    }

    #[inline(always)]
    fn ty() -> Ty {
        Ty::Array([Introspect::<T>::ty()].span())
    }
}

pub impl Introspect_span<T, +Introspect<T>> of Introspect<Span<T>> {
    #[inline(always)]
    fn size() -> Option<usize> {
        Option::None
    }
    #[inline(always)]
    fn layout() -> Layout {
        Layout::Array([Introspect::<T>::layout()].span())
    }

    #[inline(always)]
    fn ty() -> Ty {
        Ty::Array([Introspect::<T>::ty()].span())
    }
}

pub impl Introspect_FixedArray<T, const N: usize, +Introspect<T>> of Introspect<[T; N]> {
    fn size() -> Option<usize> {
        match Introspect::<T>::size() {
            Option::Some(size) => Option::Some(size * N),
            Option::None => Option::None,
        }
    }
    fn layout() -> Layout {
        Layout::FixedArray(([Introspect::<T>::layout()].span(), N))
    }
    fn ty() -> Ty {
        Ty::FixedArray(([Introspect::<T>::ty()].span(), N))
    }
}

pub impl IntrospectTupleSize0 of Introspect<()> {
    #[inline(always)]
    fn size() -> Option<usize> {
        Option::Some(0)
    }
    #[inline(always)]
    fn layout() -> Layout {
        Layout::Tuple([].span())
    }
    #[inline(always)]
    fn ty() -> Ty {
        Ty::Tuple([].span())
    }
}

pub impl IntrospectTupleSize1<E0, impl I0: Introspect<E0>> of Introspect<(E0,)> {
    #[inline(always)]
    fn size() -> Option<usize> {
        I0::size()
    }
    #[inline(always)]
    fn layout() -> Layout {
        Layout::Tuple([I0::layout()].span())
    }

    #[inline(always)]
    fn ty() -> Ty {
        Ty::Tuple([I0::ty()].span())
    }
}

pub impl IntrospectTupleSize2<
    E0, E1, impl I0: Introspect<E0>, impl I1: Introspect<E1>,
> of Introspect<(E0, E1)> {
    #[inline(always)]
    fn size() -> Option<usize> {
        sum_sizes(array![I0::size(), I1::size()])
    }

    #[inline(always)]
    fn layout() -> Layout {
        Layout::Tuple([I0::layout(), I1::layout()].span())
    }

    #[inline(always)]
    fn ty() -> Ty {
        Ty::Tuple([I0::ty(), I1::ty()].span())
    }
}

pub impl IntrospectTupleSize3<
    E0, E1, E2, impl I0: Introspect<E0>, impl I1: Introspect<E1>, impl I2: Introspect<E2>,
> of Introspect<(E0, E1, E2)> {
    #[inline(always)]
    fn size() -> Option<usize> {
        sum_sizes(array![I0::size(), I1::size(), I2::size()])
    }

    #[inline(always)]
    fn layout() -> Layout {
        Layout::Tuple([I0::layout(), I1::layout(), I2::layout()].span())
    }

    #[inline(always)]
    fn ty() -> Ty {
        Ty::Tuple([I0::ty(), I1::ty(), I2::ty()].span())
    }
}

pub impl IntrospectTupleSize4<
    E0,
    E1,
    E2,
    E3,
    impl I0: Introspect<E0>,
    impl I1: Introspect<E1>,
    impl I2: Introspect<E2>,
    impl I3: Introspect<E3>,
> of Introspect<(E0, E1, E2, E3)> {
    #[inline(always)]
    fn size() -> Option<usize> {
        sum_sizes(array![I0::size(), I1::size(), I2::size(), I3::size()])
    }

    #[inline(always)]
    fn layout() -> Layout {
        Layout::Tuple([I0::layout(), I1::layout(), I2::layout(), I3::layout()].span())
    }

    #[inline(always)]
    fn ty() -> Ty {
        Ty::Tuple([I0::ty(), I1::ty(), I2::ty(), I3::ty()].span())
    }
}

pub impl IntrospectTupleSize5<
    E0,
    E1,
    E2,
    E3,
    E4,
    impl I0: Introspect<E0>,
    impl I1: Introspect<E1>,
    impl I2: Introspect<E2>,
    impl I3: Introspect<E3>,
    impl I4: Introspect<E4>,
> of Introspect<(E0, E1, E2, E3, E4)> {
    #[inline(always)]
    fn size() -> Option<usize> {
        sum_sizes(array![I0::size(), I1::size(), I2::size(), I3::size(), I4::size()])
    }

    #[inline(always)]
    fn layout() -> Layout {
        Layout::Tuple([I0::layout(), I1::layout(), I2::layout(), I3::layout(), I4::layout()].span())
    }

    #[inline(always)]
    fn ty() -> Ty {
        Ty::Tuple([I0::ty(), I1::ty(), I2::ty(), I3::ty(), I4::ty()].span())
    }
}

pub impl IntrospectTupleSize6<
    E0,
    E1,
    E2,
    E3,
    E4,
    E5,
    impl I0: Introspect<E0>,
    impl I1: Introspect<E1>,
    impl I2: Introspect<E2>,
    impl I3: Introspect<E3>,
    impl I4: Introspect<E4>,
    impl I5: Introspect<E5>,
> of Introspect<(E0, E1, E2, E3, E4, E5)> {
    #[inline(always)]
    fn size() -> Option<usize> {
        sum_sizes(array![I0::size(), I1::size(), I2::size(), I3::size(), I4::size(), I5::size()])
    }

    #[inline(always)]
    fn layout() -> Layout {
        Layout::Tuple(
            [I0::layout(), I1::layout(), I2::layout(), I3::layout(), I4::layout(), I5::layout()]
                .span(),
        )
    }

    #[inline(always)]
    fn ty() -> Ty {
        Ty::Tuple([I0::ty(), I1::ty(), I2::ty(), I3::ty(), I4::ty(), I5::ty()].span())
    }
}


pub impl IntrospectTupleSize7<
    E0,
    E1,
    E2,
    E3,
    E4,
    E5,
    E6,
    impl I0: Introspect<E0>,
    impl I1: Introspect<E1>,
    impl I2: Introspect<E2>,
    impl I3: Introspect<E3>,
    impl I4: Introspect<E4>,
    impl I5: Introspect<E5>,
    impl I6: Introspect<E6>,
> of Introspect<(E0, E1, E2, E3, E4, E5, E6)> {
    #[inline(always)]
    fn size() -> Option<usize> {
        sum_sizes(
            array![
                I0::size(), I1::size(), I2::size(), I3::size(), I4::size(), I5::size(), I6::size(),
            ],
        )
    }

    #[inline(always)]
    fn layout() -> Layout {
        Layout::Tuple(
            [
                I0::layout(), I1::layout(), I2::layout(), I3::layout(), I4::layout(), I5::layout(),
                I6::layout(),
            ]
                .span(),
        )
    }

    #[inline(always)]
    fn ty() -> Ty {
        Ty::Tuple([I0::ty(), I1::ty(), I2::ty(), I3::ty(), I4::ty(), I5::ty(), I6::ty()].span())
    }
}


pub impl IntrospectTupleSize8<
    E0,
    E1,
    E2,
    E3,
    E4,
    E5,
    E6,
    E7,
    impl I0: Introspect<E0>,
    impl I1: Introspect<E1>,
    impl I2: Introspect<E2>,
    impl I3: Introspect<E3>,
    impl I4: Introspect<E4>,
    impl I5: Introspect<E5>,
    impl I6: Introspect<E6>,
    impl I7: Introspect<E7>,
> of Introspect<(E0, E1, E2, E3, E4, E5, E6, E7)> {
    #[inline(always)]
    fn size() -> Option<usize> {
        sum_sizes(
            array![
                I0::size(), I1::size(), I2::size(), I3::size(), I4::size(), I5::size(), I6::size(),
                I7::size(),
            ],
        )
    }

    #[inline(always)]
    fn layout() -> Layout {
        Layout::Tuple(
            [
                I0::layout(), I1::layout(), I2::layout(), I3::layout(), I4::layout(), I5::layout(),
                I6::layout(), I7::layout(),
            ]
                .span(),
        )
    }

    #[inline(always)]
    fn ty() -> Ty {
        Ty::Tuple(
            [I0::ty(), I1::ty(), I2::ty(), I3::ty(), I4::ty(), I5::ty(), I6::ty(), I7::ty()].span(),
        )
    }
}


pub impl IntrospectTupleSize9<
    E0,
    E1,
    E2,
    E3,
    E4,
    E5,
    E6,
    E7,
    E8,
    impl I0: Introspect<E0>,
    impl I1: Introspect<E1>,
    impl I2: Introspect<E2>,
    impl I3: Introspect<E3>,
    impl I4: Introspect<E4>,
    impl I5: Introspect<E5>,
    impl I6: Introspect<E6>,
    impl I7: Introspect<E7>,
    impl I8: Introspect<E8>,
> of Introspect<(E0, E1, E2, E3, E4, E5, E6, E7, E8)> {
    #[inline(always)]
    fn size() -> Option<usize> {
        sum_sizes(
            array![
                I0::size(), I1::size(), I2::size(), I3::size(), I4::size(), I5::size(), I6::size(),
                I7::size(), I8::size(),
            ],
        )
    }

    #[inline(always)]
    fn layout() -> Layout {
        Layout::Tuple(
            [
                I0::layout(), I1::layout(), I2::layout(), I3::layout(), I4::layout(), I5::layout(),
                I6::layout(), I7::layout(), I8::layout(),
            ]
                .span(),
        )
    }

    #[inline(always)]
    fn ty() -> Ty {
        Ty::Tuple(
            [
                I0::ty(), I1::ty(), I2::ty(), I3::ty(), I4::ty(), I5::ty(), I6::ty(), I7::ty(),
                I8::ty(),
            ]
                .span(),
        )
    }
}


pub impl IntrospectTupleSize10<
    E0,
    E1,
    E2,
    E3,
    E4,
    E5,
    E6,
    E7,
    E8,
    E9,
    impl I0: Introspect<E0>,
    impl I1: Introspect<E1>,
    impl I2: Introspect<E2>,
    impl I3: Introspect<E3>,
    impl I4: Introspect<E4>,
    impl I5: Introspect<E5>,
    impl I6: Introspect<E6>,
    impl I7: Introspect<E7>,
    impl I8: Introspect<E8>,
    impl I9: Introspect<E9>,
> of Introspect<(E0, E1, E2, E3, E4, E5, E6, E7, E8, E9)> {
    #[inline(always)]
    fn size() -> Option<usize> {
        sum_sizes(
            array![
                I0::size(), I1::size(), I2::size(), I3::size(), I4::size(), I5::size(), I6::size(),
                I7::size(), I8::size(), I9::size(),
            ],
        )
    }

    #[inline(always)]
    fn layout() -> Layout {
        Layout::Tuple(
            [
                I0::layout(), I1::layout(), I2::layout(), I3::layout(), I4::layout(), I5::layout(),
                I6::layout(), I7::layout(), I8::layout(), I9::layout(),
            ]
                .span(),
        )
    }

    #[inline(always)]
    fn ty() -> Ty {
        Ty::Tuple(
            [
                I0::ty(), I1::ty(), I2::ty(), I3::ty(), I4::ty(), I5::ty(), I6::ty(), I7::ty(),
                I8::ty(), I9::ty(),
            ]
                .span(),
        )
    }
}


pub impl IntrospectTupleSize11<
    E0,
    E1,
    E2,
    E3,
    E4,
    E5,
    E6,
    E7,
    E8,
    E9,
    E10,
    impl I0: Introspect<E0>,
    impl I1: Introspect<E1>,
    impl I2: Introspect<E2>,
    impl I3: Introspect<E3>,
    impl I4: Introspect<E4>,
    impl I5: Introspect<E5>,
    impl I6: Introspect<E6>,
    impl I7: Introspect<E7>,
    impl I8: Introspect<E8>,
    impl I9: Introspect<E9>,
    impl I10: Introspect<E10>,
> of Introspect<(E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10)> {
    #[inline(always)]
    fn size() -> Option<usize> {
        sum_sizes(
            array![
                I0::size(), I1::size(), I2::size(), I3::size(), I4::size(), I5::size(), I6::size(),
                I7::size(), I8::size(), I9::size(), I10::size(),
            ],
        )
    }

    #[inline(always)]
    fn layout() -> Layout {
        Layout::Tuple(
            [
                I0::layout(), I1::layout(), I2::layout(), I3::layout(), I4::layout(), I5::layout(),
                I6::layout(), I7::layout(), I8::layout(), I9::layout(), I10::layout(),
            ]
                .span(),
        )
    }

    #[inline(always)]
    fn ty() -> Ty {
        Ty::Tuple(
            [
                I0::ty(), I1::ty(), I2::ty(), I3::ty(), I4::ty(), I5::ty(), I6::ty(), I7::ty(),
                I8::ty(), I9::ty(), I10::ty(),
            ]
                .span(),
        )
    }
}

pub impl IntrospectTupleSize12<
    E0,
    E1,
    E2,
    E3,
    E4,
    E5,
    E6,
    E7,
    E8,
    E9,
    E10,
    E11,
    impl I0: Introspect<E0>,
    impl I1: Introspect<E1>,
    impl I2: Introspect<E2>,
    impl I3: Introspect<E3>,
    impl I4: Introspect<E4>,
    impl I5: Introspect<E5>,
    impl I6: Introspect<E6>,
    impl I7: Introspect<E7>,
    impl I8: Introspect<E8>,
    impl I9: Introspect<E9>,
    impl I10: Introspect<E10>,
    impl I11: Introspect<E11>,
> of Introspect<(E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11)> {
    #[inline(always)]
    fn size() -> Option<usize> {
        sum_sizes(
            array![
                I0::size(), I1::size(), I2::size(), I3::size(), I4::size(), I5::size(), I6::size(),
                I7::size(), I8::size(), I9::size(), I10::size(), I11::size(),
            ],
        )
    }

    #[inline(always)]
    fn layout() -> Layout {
        Layout::Tuple(
            [
                I0::layout(), I1::layout(), I2::layout(), I3::layout(), I4::layout(), I5::layout(),
                I6::layout(), I7::layout(), I8::layout(), I9::layout(), I10::layout(),
                I11::layout(),
            ]
                .span(),
        )
    }

    #[inline(always)]
    fn ty() -> Ty {
        Ty::Tuple(
            [
                I0::ty(), I1::ty(), I2::ty(), I3::ty(), I4::ty(), I5::ty(), I6::ty(), I7::ty(),
                I8::ty(), I9::ty(), I10::ty(), I11::ty(),
            ]
                .span(),
        )
    }
}


pub impl IntrospectTupleSize13<
    E0,
    E1,
    E2,
    E3,
    E4,
    E5,
    E6,
    E7,
    E8,
    E9,
    E10,
    E11,
    E12,
    impl I0: Introspect<E0>,
    impl I1: Introspect<E1>,
    impl I2: Introspect<E2>,
    impl I3: Introspect<E3>,
    impl I4: Introspect<E4>,
    impl I5: Introspect<E5>,
    impl I6: Introspect<E6>,
    impl I7: Introspect<E7>,
    impl I8: Introspect<E8>,
    impl I9: Introspect<E9>,
    impl I10: Introspect<E10>,
    impl I11: Introspect<E11>,
    impl I12: Introspect<E12>,
> of Introspect<(E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12)> {
    #[inline(always)]
    fn size() -> Option<usize> {
        sum_sizes(
            array![
                I0::size(), I1::size(), I2::size(), I3::size(), I4::size(), I5::size(), I6::size(),
                I7::size(), I8::size(), I9::size(), I10::size(), I11::size(), I12::size(),
            ],
        )
    }

    #[inline(always)]
    fn layout() -> Layout {
        Layout::Tuple(
            [
                I0::layout(), I1::layout(), I2::layout(), I3::layout(), I4::layout(), I5::layout(),
                I6::layout(), I7::layout(), I8::layout(), I9::layout(), I10::layout(),
                I11::layout(), I12::layout(),
            ]
                .span(),
        )
    }

    #[inline(always)]
    fn ty() -> Ty {
        Ty::Tuple(
            [
                I0::ty(), I1::ty(), I2::ty(), I3::ty(), I4::ty(), I5::ty(), I6::ty(), I7::ty(),
                I8::ty(), I9::ty(), I10::ty(), I11::ty(), I12::ty(),
            ]
                .span(),
        )
    }
}


pub impl IntrospectTupleSize14<
    E0,
    E1,
    E2,
    E3,
    E4,
    E5,
    E6,
    E7,
    E8,
    E9,
    E10,
    E11,
    E12,
    E13,
    impl I0: Introspect<E0>,
    impl I1: Introspect<E1>,
    impl I2: Introspect<E2>,
    impl I3: Introspect<E3>,
    impl I4: Introspect<E4>,
    impl I5: Introspect<E5>,
    impl I6: Introspect<E6>,
    impl I7: Introspect<E7>,
    impl I8: Introspect<E8>,
    impl I9: Introspect<E9>,
    impl I10: Introspect<E10>,
    impl I11: Introspect<E11>,
    impl I12: Introspect<E12>,
    impl I13: Introspect<E13>,
> of Introspect<(E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12, E13)> {
    #[inline(always)]
    fn size() -> Option<usize> {
        sum_sizes(
            array![
                I0::size(), I1::size(), I2::size(), I3::size(), I4::size(), I5::size(), I6::size(),
                I7::size(), I8::size(), I9::size(), I10::size(), I11::size(), I12::size(),
                I13::size(),
            ],
        )
    }

    #[inline(always)]
    fn layout() -> Layout {
        Layout::Tuple(
            [
                I0::layout(), I1::layout(), I2::layout(), I3::layout(), I4::layout(), I5::layout(),
                I6::layout(), I7::layout(), I8::layout(), I9::layout(), I10::layout(),
                I11::layout(), I12::layout(), I13::layout(),
            ]
                .span(),
        )
    }

    #[inline(always)]
    fn ty() -> Ty {
        Ty::Tuple(
            [
                I0::ty(), I1::ty(), I2::ty(), I3::ty(), I4::ty(), I5::ty(), I6::ty(), I7::ty(),
                I8::ty(), I9::ty(), I10::ty(), I11::ty(), I12::ty(), I13::ty(),
            ]
                .span(),
        )
    }
}


pub impl IntrospectTupleSize15<
    E0,
    E1,
    E2,
    E3,
    E4,
    E5,
    E6,
    E7,
    E8,
    E9,
    E10,
    E11,
    E12,
    E13,
    E14,
    impl I0: Introspect<E0>,
    impl I1: Introspect<E1>,
    impl I2: Introspect<E2>,
    impl I3: Introspect<E3>,
    impl I4: Introspect<E4>,
    impl I5: Introspect<E5>,
    impl I6: Introspect<E6>,
    impl I7: Introspect<E7>,
    impl I8: Introspect<E8>,
    impl I9: Introspect<E9>,
    impl I10: Introspect<E10>,
    impl I11: Introspect<E11>,
    impl I12: Introspect<E12>,
    impl I13: Introspect<E13>,
    impl I14: Introspect<E14>,
> of Introspect<(E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12, E13, E14)> {
    #[inline(always)]
    fn size() -> Option<usize> {
        sum_sizes(
            array![
                I0::size(), I1::size(), I2::size(), I3::size(), I4::size(), I5::size(), I6::size(),
                I7::size(), I8::size(), I9::size(), I10::size(), I11::size(), I12::size(),
                I13::size(), I14::size(),
            ],
        )
    }

    #[inline(always)]
    fn layout() -> Layout {
        Layout::Tuple(
            [
                I0::layout(), I1::layout(), I2::layout(), I3::layout(), I4::layout(), I5::layout(),
                I6::layout(), I7::layout(), I8::layout(), I9::layout(), I10::layout(),
                I11::layout(), I12::layout(), I13::layout(), I14::layout(),
            ]
                .span(),
        )
    }

    #[inline(always)]
    fn ty() -> Ty {
        Ty::Tuple(
            [
                I0::ty(), I1::ty(), I2::ty(), I3::ty(), I4::ty(), I5::ty(), I6::ty(), I7::ty(),
                I8::ty(), I9::ty(), I10::ty(), I11::ty(), I12::ty(), I13::ty(), I14::ty(),
            ]
                .span(),
        )
    }
}


pub impl IntrospectTupleSize16<
    E0,
    E1,
    E2,
    E3,
    E4,
    E5,
    E6,
    E7,
    E8,
    E9,
    E10,
    E11,
    E12,
    E13,
    E14,
    E15,
    impl I0: Introspect<E0>,
    impl I1: Introspect<E1>,
    impl I2: Introspect<E2>,
    impl I3: Introspect<E3>,
    impl I4: Introspect<E4>,
    impl I5: Introspect<E5>,
    impl I6: Introspect<E6>,
    impl I7: Introspect<E7>,
    impl I8: Introspect<E8>,
    impl I9: Introspect<E9>,
    impl I10: Introspect<E10>,
    impl I11: Introspect<E11>,
    impl I12: Introspect<E12>,
    impl I13: Introspect<E13>,
    impl I14: Introspect<E14>,
    impl I15: Introspect<E15>,
> of Introspect<(E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12, E13, E14, E15)> {
    #[inline(always)]
    fn size() -> Option<usize> {
        sum_sizes(
            array![
                I0::size(), I1::size(), I2::size(), I3::size(), I4::size(), I5::size(), I6::size(),
                I7::size(), I8::size(), I9::size(), I10::size(), I11::size(), I12::size(),
                I13::size(), I14::size(), I15::size(),
            ],
        )
    }

    #[inline(always)]
    fn layout() -> Layout {
        Layout::Tuple(
            [
                I0::layout(), I1::layout(), I2::layout(), I3::layout(), I4::layout(), I5::layout(),
                I6::layout(), I7::layout(), I8::layout(), I9::layout(), I10::layout(),
                I11::layout(), I12::layout(), I13::layout(), I14::layout(), I15::layout(),
            ]
                .span(),
        )
    }

    #[inline(always)]
    fn ty() -> Ty {
        Ty::Tuple(
            [
                I0::ty(), I1::ty(), I2::ty(), I3::ty(), I4::ty(), I5::ty(), I6::ty(), I7::ty(),
                I8::ty(), I9::ty(), I10::ty(), I11::ty(), I12::ty(), I13::ty(), I14::ty(),
                I15::ty(),
            ]
                .span(),
        )
    }
}
