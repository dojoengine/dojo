#[system]
mod spawn {
    use array::ArrayTrait;
    use box::BoxTrait;
    use option::OptionTrait;
    use traits::{Into, TryInto};
    use starknet::class_hash::{Felt252TryIntoClassHash};

    use dojo::world::Context;
    use types_test::components::Record;

    fn execute(ctx: Context, num_records: u8) {
        let mut curr_record = 0;
        loop {
            if curr_record == num_records {
                break();
            }
            curr_record = curr_record + 1;

            let record_id = ctx.world.uuid();
            let curr_felt: felt252 = curr_record.into();
            set !(
                ctx.world,
                (
                    Record {
                        record_id,
                        type_u8: curr_record.into(),
                        type_u16: curr_record.into(),
                        type_u32: curr_record.into(),
                        type_u64: curr_record.into(),
                        type_u128: curr_record.into(),
                        type_u256: curr_record.into(),
                        type_bool: if curr_record % 2  == 0 { true } else { false },
                        type_felt: curr_felt,
                        type_class_hash: curr_felt.try_into().unwrap(),
                        type_contract_address: curr_felt.try_into().unwrap(),
                    }
                )
            );
        };
        return ();
    }
}