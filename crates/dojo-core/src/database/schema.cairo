#[derive(Copy, Drop, Serde)]
enum MemberType {
    Simple: felt252,
    Complex: Span<Span<felt252>>,
    Enum: Span<felt252>
}

#[derive(Copy, Drop, Serde)]
struct Member {
    name: felt252,
    ty: MemberType,
    attrs: Span<felt252>
}

// Remove once https://github.com/starkware-libs/cairo/issues/4075 is resolved
fn serialize_member(m: @Member) -> Span<felt252> {
    let mut serialized = ArrayTrait::new();
    m.serialize(ref serialized);
    serialized.span()
}

trait SchemaIntrospection<T> {
    fn size() -> usize;
    fn layout(ref layout: Array<u8>);
    fn ty() -> MemberType;
}

impl SchemaIntrospectionFelt252 of SchemaIntrospection<felt252> {
    #[inline(always)]
    fn size() -> usize {
        1
    }

    #[inline(always)]
    fn layout(ref layout: Array<u8>) {
        // We round down felt252 since it is 251 < felt252 < 252
        layout.append(251);
    }

    #[inline(always)]
    fn ty() -> MemberType {
        MemberType::Simple('felt252')
    }
}

impl SchemaIntrospectionBool of SchemaIntrospection<bool> {
    #[inline(always)]
    fn size() -> usize {
        1
    }

    #[inline(always)]
    fn layout(ref layout: Array<u8>) {
        layout.append(1);
    }

    #[inline(always)]
    fn ty() -> MemberType {
        MemberType::Simple('bool')
    }
}

impl SchemaIntrospectionU8 of SchemaIntrospection<u8> {
    #[inline(always)]
    fn size() -> usize {
        1
    }

    #[inline(always)]
    fn layout(ref layout: Array<u8>) {
        layout.append(8);
    }

    #[inline(always)]
    fn ty() -> MemberType {
        MemberType::Simple('u8')
    }
}

impl SchemaIntrospectionU16 of SchemaIntrospection<u16> {
    #[inline(always)]
    fn size() -> usize {
        1
    }

    #[inline(always)]
    fn layout(ref layout: Array<u8>) {
        layout.append(16);
    }

    #[inline(always)]
    fn ty() -> MemberType {
        MemberType::Simple('u16')
    }
}

impl SchemaIntrospectionU32 of SchemaIntrospection<u32> {
    #[inline(always)]
    fn size() -> usize {
        1
    }

    #[inline(always)]
    fn layout(ref layout: Array<u8>) {
        layout.append(32);
    }

    #[inline(always)]
    fn ty() -> MemberType {
        MemberType::Simple('u32')
    }
}

impl SchemaIntrospectionU64 of SchemaIntrospection<u64> {
    #[inline(always)]
    fn size() -> usize {
        1
    }

    #[inline(always)]
    fn layout(ref layout: Array<u8>) {
        layout.append(64);
    }

    #[inline(always)]
    fn ty() -> MemberType {
        MemberType::Simple('u64')
    }
}

impl SchemaIntrospectionU128 of SchemaIntrospection<u128> {
    #[inline(always)]
    fn size() -> usize {
        1
    }

    #[inline(always)]
    fn layout(ref layout: Array<u8>) {
        layout.append(128);
    }

    #[inline(always)]
    fn ty() -> MemberType {
        MemberType::Simple('u128')
    }
}

impl SchemaIntrospectionU256 of SchemaIntrospection<u256> {
    #[inline(always)]
    fn size() -> usize {
        2
    }

    #[inline(always)]
    fn layout(ref layout: Array<u8>) {
        layout.append(128);
        layout.append(128);
    }

    #[inline(always)]
    fn ty() -> MemberType {
        MemberType::Simple('u256')
    }
}

impl SchemaIntrospectionContractAddress of SchemaIntrospection<starknet::ContractAddress> {
    #[inline(always)]
    fn size() -> usize {
        1
    }

    #[inline(always)]
    fn layout(ref layout: Array<u8>) {
        layout.append(251);
    }

    #[inline(always)]
    fn ty() -> MemberType {
        MemberType::Simple('ContractAddress')
    }
}

impl SchemaIntrospectionClassHash of SchemaIntrospection<starknet::ClassHash> {
    #[inline(always)]
    fn size() -> usize {
        1
    }

    #[inline(always)]
    fn layout(ref layout: Array<u8>) {
        layout.append(251);
    }

    #[inline(always)]
    fn ty() -> MemberType {
        MemberType::Simple('ClassHash')
    }
}
