#[starknet::interface]
trait IRecords<T> {
    fn create(ref self: T, num_records: u8);
    fn delete(ref self: T, record_id: u32);
}

#[dojo::contract]
mod records {
    use types_test::models::{
        Record, RecordSibling, Subrecord, Nested, NestedMore, NestedMost, Depth
    };
    use types_test::{seed, random};
    use dojo::model::ModelStorage;
    use dojo::event::EventStorage;
    use dojo::world::IWorldDispatcherTrait;
    use super::IRecords;

    #[derive(Drop, Serde, starknet::Event)]
    #[dojo::event]
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
        fn create(ref self: ContractState, num_records: u8) {
            let mut record_idx = 0;
            let mut world = self.world(@"types_test");

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

                let record_id = world.dispatcher.uuid();
                let subrecord_id = world.dispatcher.uuid();

                let record = Record {
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
                };
                let record_sibling = RecordSibling { record_id, random_u8 };
                let subrecord = Subrecord {
                    record_id, subrecord_id, type_u8: record_idx.into(), random_u8,
                };

                world.write_model(@record);
                world.write_model(@record_sibling);
                world.write_model(@subrecord);

                record_idx += 1;

                world.emit_event(
                        @RecordLogged {
                            record_id, type_u8: record_idx.into(), type_felt, random_u128
                        }
                    );
            };
        }

        // Implemment fn delete, input param: record_id
        fn delete(ref self: ContractState, record_id: u32) {
            let mut world = self.world(@"types_test");

            let record: Record = world.read_model(record_id);
            let record_sibling: RecordSibling = world.read_model(record_id);

            let subrecord_id = record_id + 1;
            let subrecord: Subrecord = world.read_model((record_id, subrecord_id));

            world.erase_model(@record);
            world.erase_model(@record_sibling);
            world.erase_model(@subrecord);
        }
    }
}
