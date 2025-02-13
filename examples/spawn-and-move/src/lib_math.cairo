#[starknet::interface]
pub trait SimpleMath<T> {
    /// Decrements the value, saturating at 0.
    fn decrement_saturating(self: @T, value: u8) -> u8;
    fn test(self: @T) -> u8;
}

#[dojo::library]
pub mod simple_math {
    use super::SimpleMath;
    use core::num::traits::SaturatingSub;

    #[abi(embed_v0)]
    impl SimpleMathImpl of SimpleMath<ContractState> {
        fn decrement_saturating(self: @ContractState, value: u8) -> u8 {
            let mut v = value;
            v.saturating_sub(1);

            v
        }

        fn test(self: @ContractState) -> u8 {
            2
        }
    }
}
