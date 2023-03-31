// Maintains state of Realms

#[system]
mod SettleSystem {
    use array::ArrayTrait;
    use traits::Into;

    use eternum::components::realm::Realm;

    fn execute(realm_id: felt252) {

        // get realm metadata
        let player_id: felt252 = starknet::get_caller_address().into();
        let player_game_id = commands::set(
            (player_id).into(),
            (Realm {
                id: realm_id,
                founder: player_id,
                armies: 0,
                resource_ids: 0,
                cities: 0,
                harbors: 0,
                rivers: 0,
                regions: 0,
                
            })
        );
        return ();
    }
}
