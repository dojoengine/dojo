use starknet::{ClassHash, SyscallResult, SyscallResultTrait};

use dojo::world::IWorldDispatcher;

#[starknet::interface]
trait IBase<T> {
    fn world(self: @T) -> IWorldDispatcher;
}

#[starknet::contract]
mod base {
    use starknet::{ClassHash, get_caller_address};

    use dojo::upgradable::{IUpgradeable, UpgradeableTrait};
    use dojo::world::IWorldDispatcher;

    #[storage]
    struct Storage {
        world_dispatcher: IWorldDispatcher,
    }

    #[constructor]
    fn constructor(ref self: ContractState) {
        self.world_dispatcher.write(IWorldDispatcher { contract_address: get_caller_address() });
    }

    #[external(v0)]
    fn world(self: @ContractState) -> IWorldDispatcher {
        self.world_dispatcher.read()
    }

    #[external(v0)]
    impl Upgradeable of IUpgradeable<ContractState> {
        /// Upgrade contract implementation to new_class_hash
        ///
        /// # Arguments
        ///
        /// * `new_class_hash` - The new implementation class hahs.
        fn upgrade(ref self: ContractState, new_class_hash: ClassHash) {
            UpgradeableTrait::upgrade(new_class_hash);
        }
    }
}
