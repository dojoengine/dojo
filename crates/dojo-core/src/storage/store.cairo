#[contract]
mod Store {
    use array::ArrayTrait;
    use array::SpanTrait;
    use traits::Into;

    use dojo_core::serde::SpanSerde;
    use dojo_core::storage::key::StorageKey;
    use dojo_core::storage::key::StorageKeyTrait;

    use dojo_core::interfaces::IComponentLibraryDispatcher;
    use dojo_core::interfaces::IComponentDispatcherTrait;

    #[event]
    fn StoreSetRecord(table_id: felt252, key: Span<felt252>, value: Span<felt252>) {}

    #[event]
    fn StoreSetField(table_id: felt252, key: Span<felt252>, offset: u8, value: Span<felt252>) {}

    fn address(table: felt252, key: StorageKey) -> starknet::StorageBaseAddress {
        starknet::storage_base_address_from_felt252(
            hash::LegacyHash::<(felt252, StorageKey)>::hash(0x420, (table, key))
        )
    }

    #[view]
    fn get(
        table: felt252,
        class_hash: starknet::ClassHash,
        key: StorageKey,
        offset: u8,
        mut length: usize
    ) -> Span<felt252> {
        let address_domain = 0_u32;
        let base = address(table, key);
        let mut value = ArrayTrait::<felt252>::new();

        if length == 0_usize {
            length = IComponentLibraryDispatcher { class_hash: class_hash }.len()
        }

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
        class_hash: starknet::ClassHash,
        storage_key: StorageKey,
        offset: u8,
        value: Span<felt252>
    ) {
        let keys = storage_key.keys();
        let length = IComponentLibraryDispatcher { class_hash: class_hash }.len();
        assert(value.len() <= length, 'Value too long');

        let address_domain = 0_u32;
        let base = address(table, storage_key);
        _set(address_domain, base, value, offset: offset);

        StoreSetRecord(table, keys, value);
        StoreSetField(table, keys, offset, value);
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
