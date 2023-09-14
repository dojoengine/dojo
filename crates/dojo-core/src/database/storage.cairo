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
