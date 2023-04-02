// Maintains state of resource production

#[system]
mod BuildLabor {
    use array::ArrayTrait;
    use traits::Into;

    use eternum::components::config::WorldConfig;

    use eternum::components::realm::Realm;
    use eternum::components::resources::Resource;
    use eternum::components::resources::Wood;


    // todo need better way to store resources
    use eternum::constants::Resources;
    use eternum::constants::WORLD_CONFIG_ID;


    use eternum::utils::math::u128_div_remainder;
    use eternum::utils::math::get_percentage_by_bp;

    #[external]
    fn execute(realm_id: felt252, resource_id: felt252, labor_units: felt252) {
        let player_id: felt252 = starknet::get_caller_address().into();

        let tx_info = starknet::get_block_info();

        // Get Config
        let world_config: WorldConfig = commands::<WorldConfig>::get(WORLD_CONFIG_ID.into());

        let current_wood = commands::<Wood>::get(realm_id.into());

        // need DRY way to do this
        // Loop over enum?
        let wood = commands::set(
            (realm_id).into(),
            (Wood { labor_balance: 0, last_update: 0, qty_built: 0, balance: 0, vault_balance: 0 })
        );
    }
}
