#[derive(Copy, Drop, Serde)]
enum Ty {
    Primitive: felt252,
    Struct: Struct,
    Enum: Enum,
    Tuple: Span<Ty>,
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

trait SchemaIntrospection<T> {
    fn size() -> usize;
    fn layout(ref layout: Array<u8>);
    fn ty() -> Ty;
}
