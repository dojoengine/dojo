
#[derive(Component)]
struct OwnershipComponent {
    balances: LegacyMap::<ContractAddress, u256>,
}