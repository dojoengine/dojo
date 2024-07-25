use dojo::model::{Layout, FieldLayout};

#[derive(Copy, Drop, Serde)]
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

#[derive(Copy, Drop, Serde)]
pub struct Struct {
    pub name: felt252,
    pub attrs: Span<felt252>,
    pub children: Span<Member>
}

#[derive(Copy, Drop, Serde)]
pub struct Enum {
    pub name: felt252,
    pub attrs: Span<felt252>,
    pub children: Span<(felt252, Ty)>
}

#[derive(Copy, Drop, Serde)]
pub struct Member {
    pub name: felt252,
    pub attrs: Span<felt252>,
    pub ty: Ty
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
        Layout::Fixed(array![251].span())
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
        Layout::Fixed(array![1].span())
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
        Layout::Fixed(array![8].span())
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
        Layout::Fixed(array![16].span())
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
        Layout::Fixed(array![32].span())
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
        Layout::Fixed(array![64].span())
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
        Layout::Fixed(array![128].span())
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
        Layout::Fixed(array![128, 128].span())
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
        Layout::Fixed(array![251].span())
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
        Layout::Fixed(array![251].span())
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
        Layout::Fixed(array![251].span())
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
        Layout::Fixed(array![251].span())
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
        Layout::Fixed(array![251].span())
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
        Layout::Fixed(array![251].span())
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
        Layout::Fixed(array![251].span())
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
            array![
                dojo::model::FieldLayout { // Some
                 selector: 0, layout: Introspect::<T>::layout() },
                dojo::model::FieldLayout { // None
                    selector: 1, layout: Layout::Fixed(array![].span())
                },
            ]
                .span()
        )
    }

    fn ty() -> Ty {
        Ty::Enum(
            Enum {
                name: 'Option<T>',
                attrs: array![].span(),
                children: array![
                    ('Some(T)', Introspect::<T>::ty()), ('None', Ty::Tuple(array![].span()))
                ]
                    .span()
            }
        )
    }
}

pub impl Introspect_array<T, +Introspect<T>> of Introspect<Array<T>> {
    fn size() -> Option<usize> {
        Option::None
    }
    fn layout() -> Layout {
        Layout::Array(array![Introspect::<T>::layout()].span())
    }

    fn ty() -> Ty {
        Ty::Array(array![Introspect::<T>::ty()].span())
    }
}

pub impl Introspect_span<T, +Introspect<T>> of Introspect<Span<T>> {
    fn size() -> Option<usize> {
        Option::None
    }
    fn layout() -> Layout {
        Layout::Array(array![Introspect::<T>::layout()].span())
    }

    fn ty() -> Ty {
        Ty::Array(array![Introspect::<T>::ty()].span())
    }
}
