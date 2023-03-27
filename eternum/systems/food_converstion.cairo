// Converts ERC1155 food tokens into storehouse

#[system]
mod RealmConversionSystem {
    use array::ArrayTrait;
    use traits::Into;  

    use eternum::components::realms::Realm;

    fn execute(realm_id: felt252) {
        // checks resource bundle exists at realm
        // convert resource into store_house
        // clears resource value
    }
}