// Maintains state of resource production

#[system]
mod BuildLaborSystem {
    use array::ArrayTrait;
    use traits::Into;  

    use eternum::components::realms::Realm;

    fn execute(realm_id: felt252, resource_id: felt252, labor_units: felt252) {

        // 1. Check owner of s_realm
        // 2. Check resource exists on Realm -> or checks if balance exists
        // 3. Builds/Adds labor

        // ?? We will need to Loop through all the resources somehowe


        // check for tick
        // call tick if needed
    }
}