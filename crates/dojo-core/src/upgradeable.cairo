use starknet::{ClassHash, SyscallResult, SyscallResultTrait};
use zeroable::Zeroable;
use result::ResultTrait;
use serde::Serde;
use clone::Clone;
use traits::PartialEq;

#[starknet::interface]
trait IUpgradeable<T> {
    fn upgrade(ref self: T, new_class_hash: ClassHash);
}

#[derive(Clone, Drop, Serde, PartialEq, starknet::Event)]
struct Upgraded {
    class_hash: ClassHash,
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
