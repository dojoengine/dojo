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


trait StorageSize<T> {
    fn unpacked_size() -> usize;
    fn packed_size() -> usize;
}

impl StorageSizeFelt252 of StorageSize<felt252> {
    #[inline(always)]
    fn unpacked_size() -> usize {
        1
    }

    #[inline(always)]
    fn packed_size() -> usize {
        252
    }
}

impl StorageSizeBool of StorageSize<bool> {
    #[inline(always)]
    fn unpacked_size() -> usize {
        1
    }

    #[inline(always)]
    fn packed_size() -> usize {
        1
    }
}

impl StorageSizeU8 of StorageSize<u8> {
    #[inline(always)]
    fn unpacked_size() -> usize {
        1
    }

    #[inline(always)]
    fn packed_size() -> usize {
        8
    }
}

impl StorageSizeU16 of StorageSize<u16> {
    #[inline(always)]
    fn unpacked_size() -> usize {
        1
    }

    #[inline(always)]
    fn packed_size() -> usize {
        16
    }
}

impl StorageSizeU32 of StorageSize<u32> {
    #[inline(always)]
    fn unpacked_size() -> usize {
        1
    }

    #[inline(always)]
    fn packed_size() -> usize {
        32
    }
}

impl StorageSizeU64 of StorageSize<u64> {
    #[inline(always)]
    fn unpacked_size() -> usize {
        1
    }

    #[inline(always)]
    fn packed_size() -> usize {
        64
    }
}

impl StorageSizeU128 of StorageSize<u128> {
    #[inline(always)]
    fn unpacked_size() -> usize {
        1
    }

    #[inline(always)]
    fn packed_size() -> usize {
        128
    }
}

impl StorageSizeU256 of StorageSize<u256> {
    #[inline(always)]
    fn unpacked_size() -> usize {
        2
    }

    #[inline(always)]
    fn packed_size() -> usize {
        256
    }
}

impl StorageSizeContractAddress of StorageSize<starknet::ContractAddress> {
    #[inline(always)]
    fn unpacked_size() -> usize {
        1
    }

    #[inline(always)]
    fn packed_size() -> usize {
        256
    }
}

impl StorageSizeClassHash of StorageSize<starknet::ClassHash> {
    #[inline(always)]
    fn unpacked_size() -> usize {
        1
    }

    #[inline(always)]
    fn packed_size() -> usize {
        256
    }
}
