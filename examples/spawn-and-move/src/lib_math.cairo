#[starknet::interface]
pub trait SimpleMath<T> {
    /// Decrements the value, saturating at 0.
    fn decrement_saturating(self: @T, value: u8) -> u8;
}

#[dojo::library]
pub mod simple_math {
    use core::num::traits::SaturatingSub;
    use super::SimpleMath;

    #[abi(embed_v0)]
    impl SimpleMathImpl of SimpleMath<ContractState> {
        fn decrement_saturating(self: @ContractState, value: u8) -> u8 {
            value.saturating_sub(1)
        }
    }
}
