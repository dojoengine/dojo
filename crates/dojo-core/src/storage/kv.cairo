mod KeyValueStore {
    use array::ArrayTrait;
    use array::SpanTrait;
    use traits::Into;

    use dojo_core::serde::SpanSerde;

    fn address(table: felt252, key: felt252) -> starknet::StorageBaseAddress {
        starknet::storage_base_address_from_felt252(
            hash::LegacyHash::<(felt252, felt252)>::hash(0x420, (table, key))
        )
    }

    #[view]
    fn get(
        table: felt252,
        key: felt252,
        offset: u8,
        mut length: usize
    ) -> Span<felt252> {
        let address_domain = 0_u32;
        let base = address(table, key);
        let mut value = ArrayTrait::<felt252>::new();
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
        match gas::withdraw_gas() {
            Option::Some(_) => {},
            Option::None(_) => {
                let mut data = ArrayTrait::new();
                data.append('Out of gas');
                panic(data);
            },
        }

        if length.into() == offset.into() {
            return ();
        }

        value.append(
            starknet::storage_read_syscall(
                address_domain, starknet::storage_address_from_base_and_offset(base, offset)
            ).unwrap_syscall()
        );

        return _get(address_domain, base, ref value, offset + 1_u8, length);
    }

    #[external]
    fn set(
        table: felt252,
        query: felt252,
        offset: u8,
        value: Span<felt252>
    ) {
        let address_domain = 0_u32;
        let base = address(table, query);
        _set(address_domain, base, value, offset: offset);
    }

    fn _set(
        address_domain: u32,
        base: starknet::StorageBaseAddress,
        mut value: Span<felt252>,
        offset: u8
    ) {
        match gas::withdraw_gas() {
            Option::Some(_) => {},
            Option::None(_) => {
                let mut data = ArrayTrait::new();
                data.append('Out of gas');
                panic(data);
            },
        }
        match value.pop_front() {
            Option::Some(v) => {
                starknet::storage_write_syscall(
                    address_domain, starknet::storage_address_from_base_and_offset(base, offset), *v
                );
                _set(address_domain, base, value, offset + 1_u8);
            },
            Option::None(_) => {},
        }
    }
}
