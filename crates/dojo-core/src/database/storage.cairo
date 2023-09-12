use array::{ArrayTrait, SpanTrait};
use option::OptionTrait;
use starknet::SyscallResultTrait;
use traits::Into;
use poseidon::poseidon_hash_span;
use serde::Serde;

fn get(address_domain: u32, keys: Span<felt252>) -> felt252 {
    let base = starknet::storage_base_address_from_felt252(poseidon_hash_span(keys));
    starknet::storage_read_syscall(address_domain, starknet::storage_address_from_base(base))
        .unwrap_syscall()
}

fn get_many(address_domain: u32, keys: Span<felt252>, offset: u8, length: usize) -> Span<felt252> {
    let base = starknet::storage_base_address_from_felt252(poseidon_hash_span(keys));
    let mut value = ArrayTrait::new();

    let mut offset = offset;
    loop {
        if length == offset.into() {
            break ();
        }

        value
            .append(
                starknet::storage_read_syscall(
                    address_domain, starknet::storage_address_from_base_and_offset(base, offset)
                )
                    .unwrap_syscall()
            );

        offset += 1;
    };

    value.span()
}

fn set(address_domain: u32, keys: Span<felt252>, value: felt252) {
    let base = starknet::storage_base_address_from_felt252(poseidon_hash_span(keys));
    starknet::storage_write_syscall(
        address_domain, starknet::storage_address_from_base(base), value
    );
}

fn set_many(address_domain: u32, keys: Span<felt252>, offset: u8, mut value: Span<felt252>) {
    let base = starknet::storage_base_address_from_felt252(poseidon_hash_span(keys));

    let mut offset = offset;
    loop {
        match value.pop_front() {
            Option::Some(v) => {
                starknet::storage_write_syscall(
                    address_domain, starknet::storage_address_from_base_and_offset(base, offset), *v
                );
                offset += 1
            },
            Option::None(_) => {
                break ();
            },
        };
    };
}


trait StorageIntrospection<T> {
    fn size() -> usize;
    fn layout(ref layout: Array<u8>);
}

impl StorageIntrospectionFelt252 of StorageIntrospection<felt252> {
    #[inline(always)]
    fn size() -> usize {
        1
    }

    #[inline(always)]
    fn layout(ref layout: Array<u8>) {
        // We round down felt252 since it is 251 < felt252 < 252
        layout.append(251);
    }
}

impl StorageIntrospectionBool of StorageIntrospection<bool> {
    #[inline(always)]
    fn size() -> usize {
        1
    }

    #[inline(always)]
    fn layout(ref layout: Array<u8>) {
        layout.append(1);
    }
}

impl StorageIntrospectionU8 of StorageIntrospection<u8> {
    #[inline(always)]
    fn size() -> usize {
        1
    }

    #[inline(always)]
    fn layout(ref layout: Array<u8>) {
        layout.append(8);
    }
}

impl StorageIntrospectionU16 of StorageIntrospection<u16> {
    #[inline(always)]
    fn size() -> usize {
        1
    }

    #[inline(always)]
    fn layout(ref layout: Array<u8>) {
        layout.append(16);
    }
}

impl StorageIntrospectionU32 of StorageIntrospection<u32> {
    #[inline(always)]
    fn size() -> usize {
        1
    }

    #[inline(always)]
    fn layout(ref layout: Array<u8>) {
        layout.append(32);
    }
}

impl StorageIntrospectionU64 of StorageIntrospection<u64> {
    #[inline(always)]
    fn size() -> usize {
        1
    }

    #[inline(always)]
    fn layout(ref layout: Array<u8>) {
        layout.append(64);
    }
}

impl StorageIntrospectionU128 of StorageIntrospection<u128> {
    #[inline(always)]
    fn size() -> usize {
        1
    }

    #[inline(always)]
    fn layout(ref layout: Array<u8>) {
        layout.append(128);
    }
}

impl StorageIntrospectionU256 of StorageIntrospection<u256> {
    #[inline(always)]
    fn size() -> usize {
        2
    }

    #[inline(always)]
    fn layout(ref layout: Array<u8>) {
        layout.append(128);
        layout.append(128);
    }
}

impl StorageIntrospectionContractAddress of StorageIntrospection<starknet::ContractAddress> {
    #[inline(always)]
    fn size() -> usize {
        1
    }

    #[inline(always)]
    fn layout(ref layout: Array<u8>) {
        layout.append(251);
    }
}

impl StorageIntrospectionClassHash of StorageIntrospection<starknet::ClassHash> {
    #[inline(always)]
    fn size() -> usize {
        1
    }

    #[inline(always)]
    fn layout(ref layout: Array<u8>) {
        layout.append(251);
    }
}
