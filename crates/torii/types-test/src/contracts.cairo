use starknet::{ContractAddress, ClassHash};

#[dojo::interface]
trait IRecords {
    fn create(ref world: IWorldDispatcher, num_records: u8);
    fn delete(ref world: IWorldDispatcher, record_id: u32);
}

#[dojo::contract]
mod records {
    use starknet::{ContractAddress, get_caller_address};
    use types_test::models::{
        Record, RecordStore, RecordSibling, RecordSiblingStore, Subrecord, SubrecordStore, Nested,
        NestedMore, NestedMost, Depth
    };
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

    #[abi(embed_v0)]
    impl RecordsImpl of IRecords<ContractState> {
        fn create(ref world: IWorldDispatcher, num_records: u8) {
            let mut record_idx = 0;

            loop {
                if record_idx == num_records {
                    break ();
                }

                let type_felt: felt252 = record_idx.into();
                let random_u8 = random(core::pedersen::pedersen(seed(), record_idx.into()), 0, 100)
                    .try_into()
                    .unwrap();
                let random_u128 = random(
                    core::pedersen::pedersen(seed(), record_idx.into()),
                    0,
                    0xffffffffffffffffffffffffffffffff_u128
                );
                let composite_u256 = u256 { low: random_u128, high: random_u128 };

                let record_id = world.uuid();
                let subrecord_id = world.uuid();

                set!(
                    world,
                    (
                        Record {
                            record_id,
                            depth: Depth::Zero,
                            type_i8: type_felt.try_into().unwrap(),
                            type_i16: type_felt.try_into().unwrap(),
                            type_i32: type_felt.try_into().unwrap(),
                            type_i64: type_felt.try_into().unwrap(),
                            type_i128: type_felt.try_into().unwrap(),
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
                            type_deeply_nested: Nested {
                                depth: Depth::One,
                                type_number: record_idx.into(),
                                type_string: type_felt,
                                type_nested_more: NestedMore {
                                    depth: Depth::Two,
                                    type_number: record_idx.into(),
                                    type_string: type_felt,
                                    type_nested_most: NestedMost {
                                        depth: Depth::Three,
                                        type_number: record_idx.into(),
                                        type_string: type_felt,
                                    }
                                }
                            },
                            type_nested_one: NestedMost {
                                depth: Depth::One, type_number: 1, type_string: 1,
                            },
                            type_nested_two: NestedMost {
                                depth: Depth::One, type_number: 2, type_string: 2,
                            },
                            random_u8,
                            random_u128,
                            composite_u256,
                        },
                        RecordSibling { record_id, random_u8 },
                        Subrecord {
                            record_id, subrecord_id, type_u8: record_idx.into(), random_u8,
                        }
                    )
                );

                record_idx += 1;

                emit!(
                    world,
                    RecordLogged { record_id, type_u8: record_idx.into(), type_felt, random_u128 }
                );
            };
        }

        // Implemment fn delete, input param: record_id
        fn delete(ref world: IWorldDispatcher, record_id: u32) {
            let world = self.world_dispatcher.read();
            let (record, record_sibling) = get!(world, record_id, (Record, RecordSibling));
            let subrecord_id = record_id + 1;
            let subrecord = get!(world, (record_id, subrecord_id), (Subrecord));
            delete!(world, (record, record_sibling, subrecord));
        }
    }
}
