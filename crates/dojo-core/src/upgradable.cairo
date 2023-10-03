use starknet::{ClassHash, SyscallResult, SyscallResultTrait};
use zeroable::Zeroable;
use result::ResultTrait;

#[starknet::interface]
trait IUpgradeable<T> {
    fn upgrade(ref self: T, new_class_hash: ClassHash);
}

trait UpgradeableTrait {
    fn upgrade(new_class_hash: ClassHash);
}

impl UpgradeableTraitImpl of UpgradeableTrait {
    fn upgrade(new_class_hash: ClassHash) {
        assert(new_class_hash.is_non_zero(), 'class_hash cannot be zero');
        starknet::replace_class_syscall(new_class_hash).unwrap_syscall();
    }
}
