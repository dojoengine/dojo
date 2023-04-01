// Maintains state of resource production

#[system]
mod BuildLaborSystem {
    use array::ArrayTrait;
    use traits::Into;

    use eternum::components::realm::Realm;
    use eternum::components::resources::Resource;
    use eternum::components::resources::Wood;

    // todo need better way to store resources
    use eternum::constants::WOOD;

    #[external]
    fn execute(realm_id: felt252, resource_id: felt252, labor_units: felt252) {
        let player_id: felt252 = starknet::get_caller_address().into();

        let tx_info = starknet::get_block_info();

        let current_wood = commands::<Wood>::get(realm_id.into());

        // need DRY way to do this
        let wood = commands::set(
            (realm_id).into(),
            (Wood { labor_balance: 0, last_update: 0, qty_built: 0, balance: 0, vault_balance: 0 })
        );
    // match resource_id {
    //     0 => {
    //         let wood = commands::set(
    //             (realm_id).into(),
    //             (Wood {
    //                 labor_balance: 0, last_update: 0, qty_built: 0, balance: 0, vault_balance: 0
    //             })
    //         );
    //     },
    //     _ => {}
    // }
    }

    // move to utils when ready
    // if labor is fully completed
    fn is_labor_completed(current_balance: u128, time_stamp: u128) -> bool {
        if (current_balance < time_stamp) {
            return true;
        } else {
            return false;
        }
    }

    fn get_vault_generated(harvest: u128) -> u128 {
        return (harvest * 250_u128) / 1000_u128;
    }

    fn get_harvestable(harvest: u128, vault_amount: u128) -> (u128, u128) {
        let harvestable = (harvest - vault_amount) % 1600_u128;
        return (harvestable, harvestable);
    }
}
