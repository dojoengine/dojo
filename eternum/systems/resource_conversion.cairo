// Converts internal Resource value to ERC1155

#[system]
mod RealmConversionSystem {
    use array::ArrayTrait;
    use traits::Into;  

    use eternum::components::realms::Realm;

    fn execute(realm_id: felt252, location: Point) {

        // 1. Check resource bundle
        // 2. Check owner
        // 3. Check Bundle at appropriate conversion center location (Point == MarketPoint)
        // 4. Call ERC1155 system and convert

    }
}