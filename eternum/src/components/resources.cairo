// Used as helper struct throughout the world
#[derive(Copy, Drop)]
struct Resource {
    labor_balance: felt252, // balance of labor still to generate - ts
    last_update: felt252, // last update of labor
    qty_built: felt252, // resource_id -> qty
    balance: felt252, // resource_id -> balance on Entity
    vault_balance: felt252, // resource_id -> balance
}


#[derive(Component)]
struct Wood {
    labor_balance: felt252, // balance of labor still to generate - ts
    last_update: felt252, // last update of labor
    qty_built: felt252, // resource_id -> qty
    balance: felt252, // resource_id -> balance on Entity
    vault_balance: felt252, // resource_id -> balance
}
// trait ResourcesTrait {
//     // population
//     fn labor_balance(self: Resources, resource_id: felt252) -> felt252; 
// }

// impl ResourcesImpl of ResourcesTrait {
//     fn labor_balance(self: Resources) -> felt252 {
//         // get value by key
//     }
// }


