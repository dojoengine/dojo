use hash::LegacyHash;
use option::OptionTrait;

use starknet::{SyscallResult, storage_access::{StorageAccess, StorageBaseAddress}};

use dojo_core::integer::u250;

#[derive(Copy, Drop, Serde)]
struct ShortString {
    inner: felt252
}

trait ShortStringTrait {
    fn new(inner: felt252) -> ShortString;
}

impl ShortStringImpl of ShortStringTrait {
    fn new(inner: felt252) -> ShortString {
        ShortString { inner }
    }
}

// Implements the PartialEq trait for ShortString.
impl ShortStringPartialEq of PartialEq<ShortString> {
    fn eq(lhs: ShortString, rhs: ShortString) -> bool {
        lhs.inner == rhs.inner
    }

    fn ne(lhs: ShortString, rhs: ShortString) -> bool {
        lhs.inner != rhs.inner
    }
}

impl Felt252IntoShortString of Into<felt252, ShortString> {
    fn into(self: felt252) -> ShortString {
        ShortString { inner: self }
    }
}

impl Felt252TryIntoShortString of TryInto<felt252, ShortString> {
    fn try_into(self: felt252) -> Option<ShortString> {
        Option::Some(ShortString { inner: self })
    }
}

impl ShortStringIntoFelt252 of Into<ShortString, felt252> {
    fn into(self: ShortString) -> felt252 {
        self.inner
    }
}

impl ShortStringIntoU250 of Into<ShortString, u250> {
    fn into(self: ShortString) -> u250 {
        u250 { inner: self.inner }
    }
}

impl LegacyHashShortString of LegacyHash<ShortString> {
    fn hash(state: felt252, value: ShortString) -> felt252 {
        LegacyHash::hash(state, ShortStringIntoFelt252::into(value))
    }
}

impl StorageAccessShortString of StorageAccess<ShortString> {
    fn read(address_domain: u32, base: StorageBaseAddress) -> SyscallResult<ShortString> {
        Result::Ok(
            Felt252TryIntoShortString::try_into(
                StorageAccess::read(address_domain, base)?
            ).expect('Not ShortString')
        )
    }
    #[inline(always)]
    fn write(
        address_domain: u32, base: StorageBaseAddress, value: ShortString
    ) -> SyscallResult<()> {
        StorageAccess::write(address_domain, base, ShortStringIntoFelt252::into(value))
    }
}
