// Maintains state of Realms

#[system]
mod SettleSystem {
    use array::ArrayTrait;
    use traits::Into;  

    use eternum::components::realms::Realm;

    fn execute(realm_id: felt252) {

        // mint s_realm
        // stake realm

        // set time staked in Realm component
        
        let owner: felt252 = starknet::get_caller_address().into();

        // let player_game_id = commands::create((game_id, (player_id)).into(), (
        //     Realm { name: name }
        // ));
    }
}