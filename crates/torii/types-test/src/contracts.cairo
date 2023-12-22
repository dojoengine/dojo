use starknet::{ContractAddress, ClassHash};

#[starknet::interface]
trait IRecords<TContractState> {
    fn create(self: @TContractState, num_records: u8);
}

#[dojo::contract]
mod records {
    use starknet::{ContractAddress, get_caller_address};
    use types_test::models::{Record, RecordSibling, Subrecord, Nested, NestedMore, NestedMoreMore, Depth};
    use types_test::{seed, random};
    use super::IRecords;

    #[event]
    #[derive(Drop, starknet::Event)]
    enum Event {
        RecordLogged: RecordLogged
    }

    #[derive(Drop, starknet::Event)]
    struct RecordLogged {
        #[key]
        record_id: u32,
        #[key]
        type_u8: u8,
        type_felt: felt252,
        random_u128: u128,
    }

    #[external(v0)]
    impl RecordsImpl of IRecords<ContractState> {
        fn create(self: @ContractState, num_records: u8) {
            let world = self.world_dispatcher.read();
            let mut record_idx = 0;

            loop {
                if record_idx == num_records {
                    break ();
                }

                let type_felt: felt252 = record_idx.into();
                let random_u8 = random(pedersen::pedersen(seed(), record_idx.into()), 0, 100)
                    .try_into()
                    .unwrap();
                let random_u128 = random(
                    pedersen::pedersen(seed(), record_idx.into()),
                    0,
                    0xffffffffffffffffffffffffffffffff_u128
                );

                let record_id = world.uuid();
                let subrecord_id = world.uuid();

                set!(
                    world,
                    (
                        Record {
                            record_id,
                            depth: Depth::Zero,
                            type_u8: record_idx.into(),
                            type_u16: record_idx.into(),
                            type_u32: record_idx.into(),
                            type_u64: record_idx.into(),
                            type_u128: record_idx.into(),
                            type_u256: type_felt.into(),
                            type_bool: if record_idx % 2 == 0 {
                                true
                            } else {
                                false
                            },
                            type_felt: record_idx.into(),
                            type_class_hash: type_felt.try_into().unwrap(),
                            type_contract_address: type_felt.try_into().unwrap(),
                            type_nested: Nested {
                                depth: Depth::One,
                                type_number: record_idx.into(),
                                type_string: type_felt,
                                type_nested_more: NestedMore {
                                    depth: Depth::Two,
                                    type_number: record_idx.into(),
                                    type_string: type_felt,
                                    type_nested_more_more: NestedMoreMore {
                                        depth: Depth::Three,
                                        type_number: record_idx.into(),
                                        type_string: type_felt,
                                    }
                                }
                            },
                            random_u8,
                            random_u128
                        },
                        RecordSibling {
                            record_id, random_u8
                        },
                        Subrecord {
                            record_id, subrecord_id, type_u8: record_idx.into(), random_u8,
                        }
                    )
                );

                record_idx += 1;

                emit!(world, RecordLogged { record_id, type_u8: record_idx.into(), type_felt, random_u128 });
            };
            return ();
        }
    }
}
