trait SerdeLen<T> {
    fn len() -> usize;
}

impl SerdeLenFelt252 of SerdeLen<felt252> {
    #[inline(always)]
    fn len() -> usize {
        1
    }
}

impl SerdeLenBool of SerdeLen<bool> {
    #[inline(always)]
    fn len() -> usize {
        1
    }
}

impl SerdeLenU8 of SerdeLen<u8> {
    #[inline(always)]
    fn len() -> usize {
        1
    }
}

impl SerdeLenU16 of SerdeLen<u16> {
    #[inline(always)]
    fn len() -> usize {
        1
    }
}

impl SerdeLenU32 of SerdeLen<u32> {
    #[inline(always)]
    fn len() -> usize {
        1
    }
}

impl SerdeLenU64 of SerdeLen<u64> {
    #[inline(always)]
    fn len() -> usize {
        1
    }
}

impl SerdeLenU128 of SerdeLen<u128> {
    #[inline(always)]
    fn len() -> usize {
        1
    }
}

impl SerdeLenU256 of SerdeLen<u256> {
    #[inline(always)]
    fn len() -> usize {
        2
    }
}

impl SerdeLenContractAddress of SerdeLen<starknet::ContractAddress> {
    #[inline(always)]
    fn len() -> usize {
        1
    }
}

impl SerdeLenClassHash of SerdeLen<starknet::ClassHash> {
    #[inline(always)]
    fn len() -> usize {
        1
    }
}
