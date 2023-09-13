use starknet::ContractAddress;
use dojo::world::{IWorldDispatcher, IWorldDispatcherTrait};

#[derive(Component, Copy, Drop, Serde)]
struct OperatorApproval {
    #[key]
    token: ContractAddress,
    #[key]
    owner: ContractAddress,
    #[key]
    operator: ContractAddress,
    approved: bool
}

trait OperatorApprovalTrait {
    fn is_approved_for_all(
        world: IWorldDispatcher,
        token: ContractAddress,
        account: ContractAddress,
        operator: ContractAddress
    ) -> bool;

    fn unchecked_set_approval_for_all(
        world: IWorldDispatcher,
        token: ContractAddress,
        owner: ContractAddress,
        operator: ContractAddress,
        approved: bool
    );
}

impl OperatorApprovalImpl of OperatorApprovalTrait {
    fn is_approved_for_all(
        world: IWorldDispatcher,
        token: ContractAddress,
        account: ContractAddress,
        operator: ContractAddress
    ) -> bool {
        let approval = get!(world, (token, account, operator), OperatorApproval);
        approval.approved
    }

    // perform safety checks before calling this fn
    fn unchecked_set_approval_for_all(
        world: IWorldDispatcher,
        token: ContractAddress,
        owner: ContractAddress,
        operator: ContractAddress,
        approved: bool
    ) {
        let mut operator_approval = get!(world, (token, owner, operator), OperatorApproval);
        operator_approval.approved = approved;
        set!(world, (operator_approval))
    }
}
