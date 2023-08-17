#[starknet::interface]
trait IContract1<T> {
    /// Sends a message to L1 contract.
    ///
    /// # Arguments
    ///
    /// * `to_address` - Contract address on L1.
    /// * `payload` - Payload of the message.
    fn send_message(ref self: T, to_address: starknet::EthAddress, payload: Span<felt252>);
}

#[starknet::contract]
mod contract_1 {

    use array::{ArrayTrait, SpanTrait};
    use traits::Into;
    use starknet::EthAddress;
    use super::IContract1;

    #[storage]
    struct Storage {}

    #[event]
    #[derive(Drop, starknet::Event)]
    enum Event {
        MessageReceivedFromL1: MessageReceivedFromL1,
    }

    #[derive(Drop, starknet::Event)]
    struct MessageReceivedFromL1 {
        #[key]
        from_address: felt252,
        payload: Span<felt252>,
    }

    /// Handles a message received from L1.
    ///
    /// Only functions that are #[l1_handler] can
    /// receive message from L1.
    ///
    /// # Arguments
    ///
    /// * `from_address` - The L1 contract sending the message.
    /// * `payload` - Message's payload.
    ///
    /// For now `from_address` is felt252, Cairo repo has
    /// in the roadmap to switch to `EthAddress`.
    /// In production, you must always check if the `from_address` is
    /// a contract you allowed to send messages, as any contract from L1
    /// can send message to any contract on L2 and vice-versa.
    #[l1_handler]
    fn msg_handler_1(
        ref self: ContractState,
        from_address: felt252,
        payload: Span<felt252>
    ) {
        // assert(from_address == ...);

        self.emit(MessageReceivedFromL1 { from_address, payload });
    }

    #[external(v0)]
    impl Contract1Impl of IContract1<ContractState> {
        fn send_message(ref self: ContractState, to_address: EthAddress, payload: Span<felt252>) {
            starknet::send_message_to_l1_syscall(to_address.into(), payload)
                .unwrap_syscall();
        }
    }

}
