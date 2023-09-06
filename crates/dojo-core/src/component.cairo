trait Component<T> {
    fn name(self: @T) -> felt252;
    fn keys(self: @T) -> Span<felt252>;
    fn values(self: @T) -> Span<felt252>;
}

#[starknet::interface]
trait INamed<T> {
    fn name(self: @T) -> felt252;
}

trait ComponentSize<T> {
    fn storage_size() -> usize;
}

impl ComponentSizeFelt252 of ComponentSize<felt252> {
    #[inline(always)]
    fn storage_size() -> usize {
        1
    }
}

impl ComponentSizeBool of ComponentSize<bool> {
    #[inline(always)]
    fn storage_size() -> usize {
        1
    }
}

impl ComponentSizeU8 of ComponentSize<u8> {
    #[inline(always)]
    fn storage_size() -> usize {
        1
    }
}

impl ComponentSizeU16 of ComponentSize<u16> {
    #[inline(always)]
    fn storage_size() -> usize {
        1
    }
}

impl ComponentSizeU32 of ComponentSize<u32> {
    #[inline(always)]
    fn storage_size() -> usize {
        1
    }
}

impl ComponentSizeU64 of ComponentSize<u64> {
    #[inline(always)]
    fn storage_size() -> usize {
        1
    }
}

impl ComponentSizeU128 of ComponentSize<u128> {
    #[inline(always)]
    fn storage_size() -> usize {
        1
    }
}

impl ComponentSizeU256 of ComponentSize<u256> {
    #[inline(always)]
    fn storage_size() -> usize {
        2
    }
}

impl ComponentSizeContractAddress of ComponentSize<starknet::ContractAddress> {
    #[inline(always)]
    fn storage_size() -> usize {
        1
    }
}

impl ComponentSizeClassHash of ComponentSize<starknet::ClassHash> {
    #[inline(always)]
    fn storage_size() -> usize {
        1
    }
}

