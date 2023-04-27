use hash::LegacyHash;
use option::OptionTrait;
use traits::Into;

use starknet::ContractAddress;
use starknet::contract_address::ContractAddressIntoFelt252;
use starknet::SyscallResult;
use starknet::storage_access::StorageAccess;
use starknet::storage_access::StorageBaseAddress;

#[derive(Copy, Drop, Serde)]
struct u250 {
    inner: felt252
}

trait u250Trait {
    fn new(inner: felt252) -> u250;
}

impl u250Impl of u250Trait {
    fn new(inner: felt252) -> u250 {
        u250 { inner }
    }
}

// Implements the PartialEq trait for u250.
impl u250PartialEq of PartialEq<u250> {
    fn eq(lhs: u250, rhs: u250) -> bool {
        lhs.inner == rhs.inner
    }

    fn ne(lhs: u250, rhs: u250) -> bool {
        lhs.inner !=rhs.inner
    }
}

impl Felt252IntoU250 of Into<felt252, u250> {
    fn into(self: felt252) -> u250 {
        u250 { inner: self }
    }
}

impl U250IntoU250 of Into<u250, u250> {
    fn into(self: u250) -> u250 {
        self
    }
}


impl Felt252TryIntoU250 of TryInto<felt252, u250> {
    fn try_into(self: felt252) -> Option<u250> {
        // TODO: Bounds check
        Option::Some(u250 { inner: self })
    }
}

impl ContractAddressIntoU250 of Into<ContractAddress, u250> {
    fn into(self: ContractAddress) -> u250 {
        u250 { inner: self.into() }
    }
}

impl u32IntoU250 of Into<u32, u250> {
    fn into(self: u32) -> u250 {
        u250 { inner: self.into() }
    }
}

impl U250IntoFelt252 of Into<u250, felt252> {
    fn into(self: u250) -> felt252 {
        self.inner
    }
}

impl LegacyHashU250 of LegacyHash<u250> {
    fn hash(state: felt252, value: u250) -> felt252 {
        LegacyHash::hash(state, U250IntoFelt252::into(value))
    }
}

impl StorageAccessU250 of StorageAccess<u250> {
    fn read(address_domain: u32, base: StorageBaseAddress) -> SyscallResult<u250> {
        Result::Ok(
            Felt252TryIntoU250::try_into(StorageAccess::<felt252>::read(
                address_domain, base
            )?).expect('StorageAccessU250 - non u250')
        )
    }
    #[inline(always)]
    fn write(address_domain: u32, base: StorageBaseAddress, value: u250) -> SyscallResult<()> {
        StorageAccess::<felt252>::write(address_domain, base, U250IntoFelt252::into(value))
    }
}
