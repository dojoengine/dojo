#[derive(Copy, Drop, Serde)]
enum Ty {
    Primitive: felt252,
    Struct: Struct,
    Enum: Enum,
    Tuple: Span<Span<felt252>>,
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
    fn size() -> usize;
    fn layout(ref layout: Array<u8>);
    fn ty() -> Ty;
}
