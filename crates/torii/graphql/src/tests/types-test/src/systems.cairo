use dojo::world::{IWorldDispatcher, IWorldDispatcherTrait};
use starknet::{ContractAddress, ClassHash};

#[starknet::interface]
trait IRecords<TContractState> {
    fn create(self: @TContractState, world: IWorldDispatcher, num_records: u8);
}

#[system]
mod records {
    use types_test::models::{Record, Nested, MoreNested};
    use super::IRecords;

    #[external(v0)]
    impl RecordsImpl of IRecords<ContractState> {
        fn create(self: @ContractState, world: IWorldDispatcher, num_records: u8) {
            let mut curr_record = 0;
            loop {
                if curr_record == num_records {
                    break ();
                }
                curr_record = curr_record + 1;
                let curr_felt: felt252 = curr_record.into();

                let record_id = world.uuid();
                set!(
                    world,
                    (Record {
                        record_id,
                        type_u8: curr_record.into(),
                        type_u16: curr_record.into(),
                        type_u32: curr_record.into(),
                        type_u64: curr_record.into(),
                        type_u128: curr_record.into(),
                        //type_u256: curr_record.into(),
                        type_bool: if curr_record % 2 == 0 {
                            true
                        } else {
                            false
                        },
                        type_felt: curr_felt,
                        type_class_hash: curr_felt.try_into().unwrap(),
                        type_contract_address: curr_felt.try_into().unwrap(),
                        type_nested: Nested {
                            record_id,
                            type_more_nested: MoreNested { record_id }
                        }
                    })
                );
            };
            return ();
        }
    }
}
