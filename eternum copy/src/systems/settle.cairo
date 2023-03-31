// Maintains state of Realms

#[system]
mod SettleSystem {
    use array::ArrayTrait;
    use traits::Into;  

    use eternum::components::realms::Realm;

    fn execute(realm_id: felt252) {

        // checks ownership of Realm ERC721
        // settles realm
        // sets RealmComponent Data
    }
}