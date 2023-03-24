
#[derive(Component)]
struct ApprovalComponent {
    approvals: LegacyMap::<(ContractAddress, ContractAddress), u256>,
}