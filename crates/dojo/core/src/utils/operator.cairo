/// # Operator Component
///
/// The goal of this component is to allow a set of operators to perform actions on the world.
///
/// The operators will be whitelisted to have the ability to modify the world's storage. We will
/// mainly gate the `set_entity` at the moment.
/// If the operator mode is enabled, only the operators will be able to perform actions on the
/// world. The Dojo permissions remain unchanged.
///
/// The operators are checked at the account level only (and not the caller level).
/// The operator mode can optionally expire at a component level.

use starknet::ContractAddress;

#[derive(Default, Serde, Drop, starknet::Store)]
pub enum OperatorMode {
    #[default]
    Disabled,
    NeverExpire,
    ExpireAt: u64,
}

/// Interface for the operator component.
#[starknet::interface]
pub trait IOperator<T> {
    /// Changes the mode of the operator component.
    ///
    /// # Arguments
    ///
    /// * `mode` - The mode of the operator.
    fn change_mode(ref self: T, mode: OperatorMode);

    /// Grants an operator to the contract.
    ///
    /// # Arguments
    ///
    /// * `operator` - The address of the operator.
    fn grant_operator(ref self: T, operator: ContractAddress);

    /// Revokes an operator from the contract.
    ///
    /// # Arguments
    ///
    /// * `operator` - The address of the operator.
    fn revoke_operator(ref self: T, operator: ContractAddress);
}

/// Component for the operator component.
#[starknet::component]
pub mod OperatorComponent {
    use starknet::ContractAddress;
    use starknet::storage::{
        Map, StorageMapReadAccess, StorageMapWriteAccess, StoragePointerReadAccess,
        StoragePointerWriteAccess,
    };
    use super::{IOperator, OperatorMode};

    #[storage]
    pub struct Storage {
        /// The operator mode.
        pub mode: OperatorMode,
        /// A map of operators and whether they are granted.
        pub operators: Map<ContractAddress, bool>,
        /// A simple owner to avoid pulling OZ for an MVP.
        pub owner: ContractAddress,
    }

    #[event]
    #[derive(Drop, starknet::Event)]
    pub enum Event {
        OperatorModeChanged: OperatorModeChanged,
        OperatorGranted: OperatorGranted,
        OperatorRevoked: OperatorRevoked,
    }

    /// Emitted when the operator mode is changed.
    #[derive(Drop, starknet::Event)]
    pub struct OperatorModeChanged {
        pub mode: OperatorMode,
    }

    /// Emitted when an operator is granted.
    #[derive(Drop, starknet::Event)]
    pub struct OperatorGranted {
        pub operator: ContractAddress,
    }

    /// Emitted when an operator is revoked.
    #[derive(Drop, starknet::Event)]
    pub struct OperatorRevoked {
        pub operator: ContractAddress,
    }

    pub mod errors {
        pub const ONLY_OWNER: felt252 = 'caller is not owner';
    }

    #[embeddable_as(OperatorImpl)]
    impl Operator<
        TContractState, +HasComponent<TContractState>, +Drop<TContractState>,
    > of IOperator<ComponentState<TContractState>> {
        fn change_mode(ref self: ComponentState<TContractState>, mode: OperatorMode) {
            self.ensure_owner();
            self.mode.write(mode);
            self.emit(Event::OperatorModeChanged(OperatorModeChanged { mode: self.mode.read() }));
        }

        fn grant_operator(ref self: ComponentState<TContractState>, operator: ContractAddress) {
            self.ensure_owner();
            self.operators.write(operator, true);
            self.emit(Event::OperatorGranted(OperatorGranted { operator }));
        }

        fn revoke_operator(ref self: ComponentState<TContractState>, operator: ContractAddress) {
            self.ensure_owner();
            self.operators.write(operator, false);
            self.emit(Event::OperatorRevoked(OperatorRevoked { operator }));
        }
    }

    #[generate_trait]
    pub impl InternalImpl<
        TContractState, +HasComponent<TContractState>, +Drop<TContractState>,
    > of InternalTrait<TContractState> {
        fn initialize(ref self: ComponentState<TContractState>, owner: ContractAddress) {
            self.owner.write(owner);
        }

        /// Asserts the caller is the owner of the operator component.
        fn ensure_owner(ref self: ComponentState<TContractState>) {
            assert(self.owner.read() == starknet::get_caller_address(), errors::ONLY_OWNER);
        }

        /// Checks if the originator of the transaction is allowed to make the call.
        ///
        /// The caller is not used because the caller is most of the time the system contract,
        /// but optimistic katana needs the account address to be whitelisted to make calls.
        ///
        /// If the operator mode is disabled, the originator is always allowed to make the call.
        /// If the operator mode is never expired, the originator is always allowed to make the
        /// call.
        /// Otherwise, the originator is allowed to make the call if it is an operator.
        fn is_call_allowed(ref self: ComponentState<TContractState>) -> bool {
            let originator = starknet::get_tx_info().unbox().account_contract_address;

            match self.mode.read() {
                OperatorMode::Disabled => true,
                OperatorMode::NeverExpire => self.operators.read(originator),
                OperatorMode::ExpireAt(expires_at) => if starknet::get_block_timestamp() < expires_at {
                    self.operators.read(originator)
                } else {
                    true
                },
            }
        }
    }
}
