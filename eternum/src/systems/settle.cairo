// Maintains state of Realms

#[system]
mod SettleSystem {
    use array::ArrayTrait;
    use traits::Into;

    use eternum::components::realm::Realm;

    fn execute(realm_id: felt252) {

        // let mut resource_ids = ArrayTrait::new();

        // let mut army_ids = ArrayTrait::new();

        // TODO army ids and resource ids

        // call metadata on Realm contract, inject into Realm struct

        // get realm metadata 
        let player_id: felt252 = starknet::get_caller_address().into();
        let player_game_id = commands::set(
            (realm_id).into(),
            (Realm {
                id: realm_id,
                founder: player_id,
                army_ids: 0,
                resource_ids: 0,
                cities: 2_u8,
                harbors: 3_u8,
                rivers: 2_u8,
                regions: 2_u8,
                
            })
        );

        // TODO: Mint S_Realm
        return ();
    }
}
