// Harvests Labor

#[system]
mod HarvestLaborSystem {
    use array::ArrayTrait;
    use traits::Into;  

    use eternum::components::resources::Wood;

    fn execute(realm_id: felt252, resource_id: felt252, labor_units: felt252) {

        // 1. Check owner of s_realm
        // 2. Check resource on Realm
        // 3. Harvest labor units
        // 4. Add resource balance to Realms
    }
}