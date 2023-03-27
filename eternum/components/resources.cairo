#[derive(Component)]
struct Resource {
    labor_balance: LegacyMap::<u256, u256>, // balance of labor still to generate - ts
    last_update: LegacyMap::<u256, u256>, // last update of labor
    qty_built: LegacyMap::<u256, u256>, // resource_id -> qty
    balance: LegacyMap::<u256, u256>, // resource_id -> balance
    vault_balance: LegacyMap::<u256, u256>, // resource_id -> balance
}
