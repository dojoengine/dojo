mod KeyValueStore {
    use array::{ArrayTrait, SpanTrait};
    use traits::Into;
    use starknet::SyscallResultTrait;
    use option::OptionTrait;

    use dojo_core::serde::SpanSerde;

    fn address(table: felt252, key: felt252) -> starknet::StorageBaseAddress {
        starknet::storage_base_address_from_felt252(hash::LegacyHash::hash(table.into(), key))
    }

    #[view]
    fn get(table: felt252, key: felt252, offset: u8, length: usize) -> Span<felt252> {
        let address_domain = 0;
        let base = address(table, key);
        let mut value = ArrayTrait::new();
        _get(address_domain, base, ref value, offset, length);
        value.span()
    }

    fn _get(
        address_domain: u32,
        base: starknet::StorageBaseAddress,
        ref value: Array<felt252>,
        offset: u8,
        length: usize
    ) {
        if length == offset.into() {
            return ();
        }

        value
            .append(
                starknet::storage_read_syscall(
                    address_domain, starknet::storage_address_from_base_and_offset(base, offset)
                )
                    .unwrap_syscall()
            );

        return _get(address_domain, base, ref value, offset + 1, length);
    }

    #[external]
    fn set(table: felt252, query: felt252, offset: u8, value: Span<felt252>) {
        let address_domain = 0;
        let base = address(table, query);
        _set(address_domain, base, value, offset: offset);
    }

    fn _set(
        address_domain: u32,
        base: starknet::StorageBaseAddress,
        mut value: Span<felt252>,
        offset: u8
    ) {
        match value.pop_front() {
            Option::Some(v) => {
                starknet::storage_write_syscall(
                    address_domain, starknet::storage_address_from_base_and_offset(base, offset), *v
                );
                _set(address_domain, base, value, offset + 1);
            },
            Option::None(_) => {},
        }
    }
}
