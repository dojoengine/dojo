use hash::LegacyHash;
use integer::BoundedInt;
use option::OptionTrait;
use traits::{Into, TryInto};

use starknet::{ContractAddress, SyscallResult, contract_address::ContractAddressIntoFelt252, storage_access::{StorageAccess, StorageBaseAddress}};

// max value of u256's high part when u250::max is converted into u256
const HIGH_BOUND: u128 = 0x3ffffffffffffffffffffffffffffff;

// temporary, until we (Scarb) catches up with Cairo
// src: https://github.com/starkware-libs/cairo/pull/3055/
impl U256TryIntoFelt252 of TryInto<u256, felt252> {
    fn try_into(self: u256) -> Option<felt252> {
        let FELT252_PRIME_HIGH = 0x8000000000000110000000000000000;
        if self.high > FELT252_PRIME_HIGH {
            return Option::None(());
        }
        if self.high == FELT252_PRIME_HIGH {
            // since FELT252_PRIME_LOW is 1.
            if self.low != 0 {
                return Option::None(());
            }
        }
        Option::Some(
            self.high.into() * 0x100000000000000000000000000000000_felt252 + self.low.into()
        )
    }
}

#[derive(Copy, Drop, Serde)]
struct u250 {
    inner: felt252
}

trait u250Trait {
    fn new(inner: felt252) -> u250;
}

impl U250Impl of u250Trait {
    fn new(inner: felt252) -> u250 {
        u250 { inner }
    }
}

// Implements the PartialEq trait for u250.
impl U250PartialEq of PartialEq<u250> {
    fn eq(lhs: u250, rhs: u250) -> bool {
        lhs.inner == rhs.inner
    }

    fn ne(lhs: u250, rhs: u250) -> bool {
        lhs.inner != rhs.inner
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
        let v: u256 = self.into();
        if v.high > HIGH_BOUND {
            return Option::None(());
        }
        Option::Some(u250 { inner: self })
    }
}

impl ContractAddressIntoU250 of Into<ContractAddress, u250> {
    fn into(self: ContractAddress) -> u250 {
        u250 { inner: self.into() }
    }
}

impl U32IntoU250 of Into<u32, u250> {
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
            Felt252TryIntoU250::try_into(
                StorageAccess::read(address_domain, base)?
            ).expect('StorageAccessU250 - non u250')
        )
    }
    #[inline(always)]
    fn write(address_domain: u32, base: StorageBaseAddress, value: u250) -> SyscallResult<()> {
        StorageAccess::write(address_domain, base, U250IntoFelt252::into(value))
    }
}

impl U250Zeroable of Zeroable<u250> {
    #[inline(always)]
    fn zero() -> u250 {
        u250 { inner: 0 }
    }

    #[inline(always)]
    fn is_zero(self: u250) -> bool {
        self.inner == 0
    }

    #[inline(always)]
    fn is_non_zero(self: u250) -> bool {
        self.inner != 0
    }
}

impl U250BoundedInt of BoundedInt<u250> {
    #[inline(always)]
    fn min() -> u250 nopanic {
        u250 { inner: 0 }
    }

    #[inline(always)]
    fn max() -> u250 nopanic {
        // 2^250 - 1
        u250 { inner: 0x3ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff }
    }
}

impl U250Add of Add<u250> {
    #[inline(always)]
    fn add(lhs: u250, rhs: u250) -> u250 {
        let r = u250 { inner: lhs.inner + rhs.inner };
        let r256: u256 = r.inner.into();
        assert(r256.high <= HIGH_BOUND, 'u250 overflow');
        r
    }
}

impl U250AddEq of AddEq<u250> {
    #[inline(always)]
    fn add_eq(ref self: u250, other: u250) {
        self = self + other;
    }
}

impl U250Sub of Sub<u250> {
    #[inline(always)]
    fn sub(lhs: u250, rhs: u250) -> u250 {
        let lhs256: u256 = lhs.inner.into();
        let rhs256: u256 = rhs.inner.into();
        assert(lhs256 >= rhs256, 'u250 underflow');
        u250 { inner: (lhs256 - rhs256).try_into().unwrap() }
    }
}

impl U250SubEq of SubEq<u250> {
    #[inline(always)]
    fn sub_eq(ref self: u250, other: u250) {
        self = self - other;
    }
}
