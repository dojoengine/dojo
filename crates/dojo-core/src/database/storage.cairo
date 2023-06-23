use array::{ArrayTrait, SpanTrait};
use option::OptionTrait;
use starknet::SyscallResultTrait;
use traits::Into;
use poseidon::poseidon_hash_span;
use serde::Serde;
use dojo_core::serde::SpanSerde;

fn get(address_domain: u32, keys: Span<felt252>) -> felt252 {
    let base = starknet::storage_base_address_from_felt252(poseidon_hash_span(keys));
    starknet::storage_read_syscall(
        address_domain, starknet::storage_address_from_base(base)
    ).unwrap_syscall()
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
                ).unwrap_syscall()
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
