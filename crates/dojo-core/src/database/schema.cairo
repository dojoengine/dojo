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

[
0    FieldElement { inner: 0x0000000000000000000000000000000000000000000000000000000000000018 }, // len
1    FieldElement { inner: 0x0000000000000000000000000000000000000000000000000000000000000017 }, // len
2    FieldElement { inner: 0x0000000000000000000000000000000000000000000000000000000000000001 }, // member type complex
3    FieldElement { inner: 0x0000000000000000000000000000000000000000000000000000000000000002 }, // 
4    FieldElement { inner: 0x0000000000000000000000000000000000000000000000000000000000000005 }, // complex len
    5    FieldElement { inner: 0x0000000000000000000000000000000000000000000000000000706c61796572 }, // name (player)
6    FieldElement { inner: 0x0000000000000000000000000000000000000000000000000000000000000000 }, // member type simple
7    FieldElement { inner: 0x0000000000000000000000000000000000436f6e747261637441646472657373 }, // member type value (ContractAddress)
8    FieldElement { inner: 0x0000000000000000000000000000000000000000000000000000000000000001 }, // attrs len
9    FieldElement { inner: 0x00000000000000000000000000000000000000000000000000000000006b6579 }, // key
10    FieldElement { inner: 0x000000000000000000000000000000000000000000000000000000000000000e }, // len
11    FieldElement { inner: 0x0000000000000000000000000000000000000000000000000000000000766563 }, // name (vec)
12    FieldElement { inner: 0x0000000000000000000000000000000000000000000000000000000000000001 }, // complex type
13    FieldElement { inner: 0x0000000000000000000000000000000000000000000000000000000000000002 }, // len
14    FieldElement { inner: 0x0000000000000000000000000000000000000000000000000000000000000004 }, // len
15    FieldElement { inner: 0x0000000000000000000000000000000000000000000000000000000000000078 }, // name (x)
16    FieldElement { inner: 0x0000000000000000000000000000000000000000000000000000000000000000 }, // member type simple
17    FieldElement { inner: 0x0000000000000000000000000000000000000000000000000000000000753332 }, // u32
18    FieldElement { inner: 0x0000000000000000000000000000000000000000000000000000000000000000 }, // attrs len
19    FieldElement { inner: 0x0000000000000000000000000000000000000000000000000000000000000004 }, // len
20    FieldElement { inner: 0x0000000000000000000000000000000000000000000000000000000000000079 }, // name (y)
21    FieldElement { inner: 0x0000000000000000000000000000000000000000000000000000000000000000 }, // member type simple
22    FieldElement { inner: 0x0000000000000000000000000000000000000000000000000000000000753332 }, // u32
23    FieldElement { inner: 0x0000000000000000000000000000000000000000000000000000000000000000 }, // attrs len
24    FieldElement { inner: 0x0000000000000000000000000000000000000000000000000000000000000000 } // ?
]

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
