use array::{ArrayTrait, SpanTrait};
use option::OptionTrait;
use starknet::SyscallResultTrait;
use traits::Into;
use poseidon::poseidon_hash_span;
use serde::Serde;
use dojo::packing::{pack, unpack};

fn get(address_domain: u32, keys: Span<felt252>) -> felt252 {
    let base = starknet::storage_base_address_from_felt252(poseidon_hash_span(keys));
    starknet::storage_read_syscall(address_domain, starknet::storage_address_from_base(base))
        .unwrap_syscall()
}

fn get_many(address_domain: u32, keys: Span<felt252>, offset: u8, length: usize, mut layout: Span<u8>) -> Span<felt252> {
    let base = starknet::storage_base_address_from_felt252(poseidon_hash_span(keys));
    let mut packed = ArrayTrait::new();

    let mut offset = offset;
    loop {
        if length == offset.into() {
            break ();
        }

        packed
            .append(
                starknet::storage_read_syscall(
                    address_domain, starknet::storage_address_from_base_and_offset(base, offset)
                )
                    .unwrap_syscall()
            );

        offset += 1;
    };

    let mut packed = packed.span();
    let mut unpacked = ArrayTrait::new();
    unpack(ref unpacked, ref packed, ref layout);

    unpacked.span()
}

fn set(address_domain: u32, keys: Span<felt252>, value: felt252) {
    let base = starknet::storage_base_address_from_felt252(poseidon_hash_span(keys));
    starknet::storage_write_syscall(
        address_domain, starknet::storage_address_from_base(base), value
    );
}

fn set_many(address_domain: u32, keys: Span<felt252>, offset: u8, mut unpacked: Span<felt252>, mut layout: Span<u8>) {
    let base = starknet::storage_base_address_from_felt252(poseidon_hash_span(keys));

    let mut packed = ArrayTrait::new();
    pack(ref packed, ref unpacked, ref layout);

    let mut offset = offset;
    loop {
        match packed.pop_front() {
            Option::Some(v) => {
                starknet::storage_write_syscall(
                    address_domain, starknet::storage_address_from_base_and_offset(base, offset), v
                );
                offset += 1
            },
            Option::None(_) => {
                break ();
            },
        };
    };
}

#[derive(Copy, Drop, Serde)]
struct Member {
    name: felt252,
    ty: felt252
}

trait SchemaIntrospection<T> {
    fn size() -> usize;
    fn layout(ref layout: Array<u8>);
    fn schema(ref schema: Array<Member>);
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
    fn schema(ref schema: Array<Member>) {
        Member {
            name: '-',
            ty: 'felt252'
        };
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
    fn schema(ref schema: Array<Member>) {
        Member {
            name: '-',
            ty: 'bool'
        };
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
    fn schema(ref schema: Array<Member>) {
        Member {
            name: '-',
            ty: 'u8'
        };
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
    fn schema(ref schema: Array<Member>) {
        Member {
            name: '-',
            ty: 'u16'
        };
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
    fn schema(ref schema: Array<Member>) {
        Member {
            name: '-',
            ty: 'u32'
        };
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
    fn schema(ref schema: Array<Member>) {
        Member {
            name: '-',
            ty: 'u64'
        };
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
    fn schema(ref schema: Array<Member>) {
        Member {
            name: '-',
            ty: 'u128'
        };
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
    fn schema(ref schema: Array<Member>) {
        Member {
            name: '-',
            ty: 'u256'
        };
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
    fn schema(ref schema: Array<Member>) {
        Member {
            name: '-',
            ty: 'starknet::ContractAddress'
        };
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
    fn schema(ref schema: Array<Member>) {
        Member {
            name: '-',
            ty: 'starknet::ClassHash'
        };
    }
}
