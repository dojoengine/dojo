#[derive(Copy, Drop, Serde, Debug, PartialEq)]
struct FieldLayout {
    selector: felt252,
    layout: Layout
}

#[derive(Copy, Drop, Serde, Debug, PartialEq)]
enum Layout {
    Fixed: Span<u8>,
    Struct: Span<FieldLayout>,
    Tuple: Span<Layout>,
    // We can't have `Layout` here as it will cause infinite recursion.
    // And `Box` is not serializable. So using a Span, even if it's to have
    // one element, does the trick.
    Array: Span<Layout>,
    ByteArray,
    // there is one layout per variant.
    // the `selector` field identifies the variant
    // the `layout` defines the variant data (could be empty for variant without data).
    Enum: Span<FieldLayout>,
}

#[derive(Copy, Drop, Serde)]
enum Ty {
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
struct Struct {
    name: felt252,
    attrs: Span<felt252>,
    children: Span<Member>
}

#[derive(Copy, Drop, Serde)]
struct Enum {
    name: felt252,
    attrs: Span<felt252>,
    children: Span<(felt252, Ty)>
}

#[derive(Copy, Drop, Serde)]
struct Member {
    name: felt252,
    attrs: Span<felt252>,
    ty: Ty
}

trait Introspect<T> {
    fn size() -> Option<usize>;
    fn layout() -> Layout;
    fn ty() -> Ty;
}

impl Introspect_felt252 of Introspect<felt252> {
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

impl Introspect_bool of Introspect<bool> {
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

impl Introspect_u8 of Introspect<u8> {
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

impl Introspect_u16 of Introspect<u16> {
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

impl Introspect_u32 of Introspect<u32> {
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

impl Introspect_u64 of Introspect<u64> {
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

impl Introspect_u128 of Introspect<u128> {
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

impl Introspect_u256 of Introspect<u256> {
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

impl Introspect_address of Introspect<starknet::ContractAddress> {
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

impl Introspect_classhash of Introspect<starknet::ClassHash> {
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

impl Introspect_bytearray of Introspect<ByteArray> {
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

impl Introspect_option<T, +Introspect<T>> of Introspect<Option<T>> {
    fn size() -> Option<usize> {
        Option::None
    }

    fn layout() -> Layout {
        Layout::Enum(
            array![
                FieldLayout { // Some
                 selector: 0, layout: Introspect::<T>::layout() },
                FieldLayout { // None
                 selector: 1, layout: Layout::Fixed(array![].span()) },
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

impl Introspect_array<T, +Introspect<T>> of Introspect<Array<T>> {
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

impl Introspect_span<T, +Introspect<T>> of Introspect<Span<T>> {
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
