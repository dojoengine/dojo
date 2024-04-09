#[derive(Copy, Drop, Serde, Debug)]
struct FieldLayout {
    selector: felt252,
    layout: Layout
}

// #[derive(Copy, Drop, Serde, Debug)]
// struct ItemLayout {
//     layout: Layout  // booo bad compiler ! bad !
// }

#[derive(Copy, Drop, Serde, Debug)]
enum Layout {
    Fixed: Span<u8>,
    Struct: Span<FieldLayout>,

    // Use a direct reference to `Layout` through `Span<Layout>` leads to a recursion in 
    // the Layout type definition. This recursion should be supported
    // by the Cairo compiler but it does not work in Dojo.
    // That's why we use an intermediate `ItemLayout` type for Tuple.
    Tuple: Span<FieldLayout>,

    // As for Tuple, a direct reference to Layout does not work.
    // A direct reference to ItemLayout or through Box<Layout> (or Box<ItemLayout>)
    // does not work either.
    // So, even for Array, we use a `Span<ItemLayout>` which contains only one item,
    // which is the layout definition of an array item.
    Array: Span<FieldLayout>,

    ByteArray,
}

#[derive(Copy, Drop, Serde)]
enum Ty {
    Primitive: felt252,
    Struct: Struct,
    Enum: Enum,
    Tuple: Span<Span<felt252>>,
    // Store the capacity of the array.
    FixedSizeArray: u32,
    DynamicSizeArray,
}

#[derive(Copy, Drop, Serde)]
struct Struct {
    name: felt252,
    attrs: Span<felt252>,
    children: Span<Span<felt252>>
}

#[derive(Copy, Drop, Serde)]
struct Enum {
    name: felt252,
    attrs: Span<felt252>,
    children: Span<(felt252, Span<felt252>)>
}

#[derive(Copy, Drop, Serde)]
struct Member {
    name: felt252,
    attrs: Span<felt252>,
    ty: Ty
}

// Remove once https://github.com/starkware-libs/cairo/issues/4075 is resolved
fn serialize_member(m: @Member) -> Span<felt252> {
    let mut serialized = ArrayTrait::new();
    m.serialize(ref serialized);
    serialized.span()
}

// Remove once https://github.com/starkware-libs/cairo/issues/4075 is resolved
fn serialize_member_type(m: @Ty) -> Span<felt252> {
    let mut serialized = ArrayTrait::new();
    m.serialize(ref serialized);
    serialized.span()
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

impl Introspect_usize of Introspect<usize> {
    fn size() -> Option<usize> {
        Introspect_u32::size()
    }
    fn layout() -> Layout {
        Introspect_u32::layout()
    }
    fn ty() -> Ty {
        Ty::Primitive('usize')
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
        Ty::FixedSizeArray(2)
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
        Ty::DynamicSizeArray
    }
}
