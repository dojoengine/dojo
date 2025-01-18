use dojo::meta::Layout;
use dojo::storage::packing;

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

        let mut allowed_upgrades: Span<(felt252, Span<felt252>)> = [
            ('bool', [].span()), ('u8', ['u16', 'u32', 'usize', 'u64', 'u128', 'felt252'].span()),
            ('u16', ['u32', 'usize', 'u64', 'u128', 'felt252'].span()),
            ('u32', ['usize', 'u64', 'u128', 'felt252'].span()),
            ('usize', ['u32', 'u64', 'u128', 'felt252'].span()),
            ('u64', ['u128', 'felt252'].span()), ('u128', ['felt252'].span()), ('u256', [].span()),
            ('i8', ['i16', 'i32', 'i64', 'i128', 'felt252'].span()),
            ('i16', ['i32', 'i64', 'i128', 'felt252'].span()),
            ('i32', ['i64', 'i128', 'felt252'].span()), ('i64', ['i128', 'felt252'].span()),
            ('i128', ['felt252'].span()), ('felt252', ['ClassHash', 'ContractAddress'].span()),
            ('ClassHash', ['felt252', 'ContractAddress'].span()),
            ('ContractAddress', ['felt252', 'ClassHash'].span()),
        ]
            .span();

        loop {
            match allowed_upgrades.pop_front() {
                Option::Some((
                    src, allowed,
                )) => {
                    if src == old {
                        let mut i = 0;
                        break loop {
                            if i >= (*allowed).len() {
                                break false;
                            }
                            if (*allowed).at(i) == self {
                                break true;
                            }
                            i += 1;
                        };
                    }
                },
                Option::None => { break false; },
            }
        }
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

                if n.len() != o.len() {
                    return false;
                }

                let mut i = 0;
                loop {
                    if i >= n.len() {
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

            let (_, old_ty) = *old.children[i];
            let (_, new_ty) = *self.children[i];

            // changing name is acceptable as it has no impact on storage

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
        self.name == old.name && self.attrs == old.attrs && self.ty.is_an_upgrade_of(old.ty)
    }
}

pub trait Introspect<T> {
    fn size() -> Option<usize>;
    fn layout() -> Layout;
    fn ty() -> Ty;
}

pub impl Introspect_felt252 of Introspect<felt252> {
    fn size() -> Option<usize> {
        Option::Some(1)
    }
    fn layout() -> Layout {
        Layout::Fixed([packing::PACKING_MAX_BITS].span())
    }
    fn ty() -> Ty {
        Ty::Primitive('felt252')
    }
}

pub impl Introspect_bool of Introspect<bool> {
    fn size() -> Option<usize> {
        Option::Some(1)
    }
    fn layout() -> Layout {
        Layout::Fixed([1].span())
    }
    fn ty() -> Ty {
        Ty::Primitive('bool')
    }
}

pub impl Introspect_u8 of Introspect<u8> {
    fn size() -> Option<usize> {
        Option::Some(1)
    }
    fn layout() -> Layout {
        Layout::Fixed([8].span())
    }
    fn ty() -> Ty {
        Ty::Primitive('u8')
    }
}

pub impl Introspect_u16 of Introspect<u16> {
    fn size() -> Option<usize> {
        Option::Some(1)
    }
    fn layout() -> Layout {
        Layout::Fixed([16].span())
    }
    fn ty() -> Ty {
        Ty::Primitive('u16')
    }
}

pub impl Introspect_u32 of Introspect<u32> {
    fn size() -> Option<usize> {
        Option::Some(1)
    }
    fn layout() -> Layout {
        Layout::Fixed([32].span())
    }
    fn ty() -> Ty {
        Ty::Primitive('u32')
    }
}

pub impl Introspect_u64 of Introspect<u64> {
    fn size() -> Option<usize> {
        Option::Some(1)
    }
    fn layout() -> Layout {
        Layout::Fixed([64].span())
    }
    fn ty() -> Ty {
        Ty::Primitive('u64')
    }
}

pub impl Introspect_u128 of Introspect<u128> {
    fn size() -> Option<usize> {
        Option::Some(1)
    }
    fn layout() -> Layout {
        Layout::Fixed([128].span())
    }
    fn ty() -> Ty {
        Ty::Primitive('u128')
    }
}

pub impl Introspect_u256 of Introspect<u256> {
    fn size() -> Option<usize> {
        Option::Some(2)
    }
    fn layout() -> Layout {
        Layout::Fixed([128, 128].span())
    }
    fn ty() -> Ty {
        Ty::Primitive('u256')
    }
}

pub impl Introspect_i8 of Introspect<i8> {
    fn size() -> Option<usize> {
        Option::Some(1)
    }
    fn layout() -> Layout {
        Layout::Fixed([packing::PACKING_MAX_BITS].span())
    }
    fn ty() -> Ty {
        Ty::Primitive('i8')
    }
}

pub impl Introspect_i16 of Introspect<i16> {
    fn size() -> Option<usize> {
        Option::Some(1)
    }
    fn layout() -> Layout {
        Layout::Fixed([packing::PACKING_MAX_BITS].span())
    }
    fn ty() -> Ty {
        Ty::Primitive('i16')
    }
}

pub impl Introspect_i32 of Introspect<i32> {
    fn size() -> Option<usize> {
        Option::Some(1)
    }
    fn layout() -> Layout {
        Layout::Fixed([packing::PACKING_MAX_BITS].span())
    }
    fn ty() -> Ty {
        Ty::Primitive('i32')
    }
}

pub impl Introspect_i64 of Introspect<i64> {
    fn size() -> Option<usize> {
        Option::Some(1)
    }
    fn layout() -> Layout {
        Layout::Fixed([packing::PACKING_MAX_BITS].span())
    }
    fn ty() -> Ty {
        Ty::Primitive('i64')
    }
}

pub impl Introspect_i128 of Introspect<i128> {
    fn size() -> Option<usize> {
        Option::Some(1)
    }
    fn layout() -> Layout {
        Layout::Fixed([packing::PACKING_MAX_BITS].span())
    }
    fn ty() -> Ty {
        Ty::Primitive('i128')
    }
}

pub impl Introspect_address of Introspect<starknet::ContractAddress> {
    fn size() -> Option<usize> {
        Option::Some(1)
    }
    fn layout() -> Layout {
        Layout::Fixed([packing::PACKING_MAX_BITS].span())
    }
    fn ty() -> Ty {
        Ty::Primitive('starknet::ContractAddress')
    }
}

pub impl Introspect_classhash of Introspect<starknet::ClassHash> {
    fn size() -> Option<usize> {
        Option::Some(1)
    }
    fn layout() -> Layout {
        Layout::Fixed([packing::PACKING_MAX_BITS].span())
    }
    fn ty() -> Ty {
        Ty::Primitive('starknet::ClassHash')
    }
}

pub impl Introspect_bytearray of Introspect<ByteArray> {
    fn size() -> Option<usize> {
        Option::None
    }
    fn layout() -> Layout {
        Layout::ByteArray
    }
    fn ty() -> Ty {
        Ty::ByteArray
    }
}

pub impl Introspect_option<T, +Introspect<T>> of Introspect<Option<T>> {
    fn size() -> Option<usize> {
        Option::None
    }

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
    fn size() -> Option<usize> {
        Option::None
    }
    fn layout() -> Layout {
        Layout::Array([Introspect::<T>::layout()].span())
    }

    fn ty() -> Ty {
        Ty::Array([Introspect::<T>::ty()].span())
    }
}

pub impl Introspect_span<T, +Introspect<T>> of Introspect<Span<T>> {
    fn size() -> Option<usize> {
        Option::None
    }
    fn layout() -> Layout {
        Layout::Array([Introspect::<T>::layout()].span())
    }

    fn ty() -> Ty {
        Ty::Array([Introspect::<T>::ty()].span())
    }
}
