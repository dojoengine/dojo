/// This file contains the implementation of the `Introspect` trait.
///
/// The introspection is used to get the size and layout of a type.
/// It is important to note that signed integers in Cairo are always using `252` bits.

use dojo::meta::Layout;
use dojo::storage::packing;
use core::panics::panic_with_byte_array;

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

#[derive(Copy, Drop, Serde, Debug, PartialEq)]
pub enum Ty {
    Primitive: felt252,
    Struct: Struct,
    Enum: Enum,
    Tuple: Span<Ty>,
    // We can't have `Ty` here as it will cause infinite recursion.
    // And `Box` is not serializable. So using a Span, even if it's to have
    // one element, does the trick.
    Array: Span<Ty>,
    ByteArray,
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
    fn is_an_upgrade_of(self: @T, old: @T) -> bool;
}

impl PrimitiveCompareImpl of TyCompareTrait<felt252> {
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
    fn is_an_upgrade_of(self: @Ty, old: @Ty) -> bool {
        match (self, old) {
            (Ty::Primitive(n), Ty::Primitive(o)) => n.is_an_upgrade_of(o),
            (Ty::Struct(n), Ty::Struct(o)) => n.is_an_upgrade_of(o),
            (Ty::Array(n), Ty::Array(o)) => { (*n).at(0).is_an_upgrade_of((*o).at(0)) },
            (
                Ty::Tuple(n), Ty::Tuple(o),
            ) => {
                let n = *n;
                let o = *o;

                if n.len() < o.len() {
                    return false;
                }

                let mut i = 0;
                loop {
                    if i >= o.len() {
                        break true;
                    }
                    if !n.at(i).is_an_upgrade_of(o.at(i)) {
                        break false;
                    }
                    i += 1;
                }
            },
            (Ty::ByteArray, Ty::ByteArray) => true,
            (Ty::Enum(n), Ty::Enum(o)) => n.is_an_upgrade_of(o),
            _ => false,
        }
    }
}

impl EnumCompareImpl of TyCompareTrait<Enum> {
    fn is_an_upgrade_of(self: @Enum, old: @Enum) -> bool {
        if self.name != old.name
            || self.attrs != old.attrs
            || (*self.children).len() < (*old.children).len() {
            return false;
        }

        let mut i = 0;

        loop {
            if i >= (*old.children).len() {
                break true;
            }

            let (old_name, old_ty) = *old.children[i];
            let (new_name, new_ty) = *self.children[i];

            // renaming is not allowed as checking if variants have not been reordered
            // could be quite challenging
            if new_name != old_name {
                break false;
            }

            if !new_ty.is_an_upgrade_of(@old_ty) {
                break false;
            }

            i += 1;
        }
    }
}

impl StructCompareImpl of TyCompareTrait<Struct> {
    fn is_an_upgrade_of(self: @Struct, old: @Struct) -> bool {
        if self.name != old.name
            || self.attrs != old.attrs
            || (*self.children).len() < (*old.children).len() {
            return false;
        }

        let mut i = 0;

        loop {
            if i >= (*old.children).len() {
                break true;
            }

            if !self.children[i].is_an_upgrade_of(old.children[i]) {
                break false;
            }

            i += 1;
        }
    }
}

impl MemberCompareImpl of TyCompareTrait<Member> {
    fn is_an_upgrade_of(self: @Member, old: @Member) -> bool {
        if self.name != old.name || self.attrs != old.attrs {
            return false;
        }

        let mut i = 0;
        let is_key = loop {
            if i >= (*self).attrs.len() {
                break false;
            }

            if *self.attrs[i] == 'key' {
                break true;
            }

            i += 1;
        };

        if is_key {
            match (self.ty, old.ty) {
                (Ty::Primitive(n), Ty::Primitive(o)) => n.is_an_upgrade_of(o),
                (
                    Ty::Enum(n), Ty::Enum(o),
                ) => {
                    if n == o {
                        return true;
                    }

                    let n = *n;
                    let o = *o;

                    if n.name != o.name
                        || n.attrs != o.attrs
                        || n.children.len() < o.children.len() {
                        return false;
                    }

                    // only new variants are allowed so existing variants must remain
                    // the same.
                    let mut i = 0;
                    loop {
                        if i >= o.children.len() {
                            break true;
                        }

                        let (new_name, new_ty) = n.children[i];
                        let (old_name, old_ty) = o.children[i];

                        if new_name != old_name || new_ty != old_ty {
                            break false;
                        }

                        i += 1;
                    }
                },
                (Ty::Struct(n), Ty::Struct(o)) => n == o,
                (Ty::Array(n), Ty::Array(o)) => n == o,
                (Ty::Tuple(n), Ty::Tuple(o)) => n == o,
                (Ty::ByteArray, Ty::ByteArray) => true,
                _ => false,
            }
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
                selector: 0, layout: Introspect::<T>::layout() },
                dojo::meta::FieldLayout { // None
                selector: 1, layout: Layout::Fixed([].span()) },
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
