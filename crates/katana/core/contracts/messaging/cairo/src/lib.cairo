use starknet::EthAddress;
use serde::Serde;

// A custom serializable struct.
#[derive(Drop, Serde)]
struct MyData {
    a: felt252,
    b: felt252,
}

#[starknet::interface]
trait IContract1<T> {
    /// Sends a message to L1 contract with a single felt252 value.
    ///
    /// # Arguments
    ///
    /// * `to_address` - Contract address on L1.
    /// * `value` - Value to be sent in the payload.
    fn send_message_value(ref self: T, to_address: EthAddress, value: felt252);

    /// Sends a message to L1 contract with a serialized struct.
    ///
    /// # Arguments
    ///
    /// * `to_address` - Contract address on L1.
    /// * `data` - Data to be sent in the payload.
    fn send_message_struct(ref self: T, to_address: EthAddress, data: MyData);
}

#[starknet::contract]
mod contract_1 {
    use array::{ArrayTrait, SpanTrait};
    use traits::Into;
    use starknet::EthAddress;
    use serde::Serde;
    use super::{IContract1, MyData};

    #[storage]
    struct Storage {}

    /// Handles a message received from L1.
    ///
    /// Only functions that are #[l1_handler] can
    /// receive message from L1.
    ///
    /// # Arguments
    ///
    /// * `from_address` - The L1 contract sending the message.
    /// * `value` - Expected value in the payload (automatically deserialized).
    ///
    /// In production, you must always check if the `from_address` is
    /// a contract you allowed to send messages, as any contract from L1
    /// can send message to any contract on L2 and vice-versa.
    ///
    /// In this example, the payload is expected to be a single felt value. But it can be any
    /// deserializable struct written in cairo.
    #[l1_handler]
    fn msg_handler_value(ref self: ContractState, from_address: felt252, value: felt252) {
        // assert(from_address == ...);

        assert(value == 123, 'Invalid value');
    }

    /// Handles a message received from L1.
    ///
    /// # Arguments
    ///
    /// * `from_address` - The L1 contract sending the message.
    /// * `data` - Expected data in the payload (automatically deserialized).
    #[l1_handler]
    fn msg_handler_struct(ref self: ContractState, from_address: felt252, data: MyData) {
        // assert(from_address == ...);

        assert(data.a == 0, 'data.a is invalid');
        assert(data.b == 0, 'data.b is invalid');
    }

    #[external(v0)]
    impl Contract1Impl of IContract1<ContractState> {
        fn send_message_value(ref self: ContractState, to_address: EthAddress, value: felt252) {
            // Note here, we "serialized" the felt252 value.
            starknet::send_message_to_l1_syscall(to_address.into(), array![value].span())
                .unwrap_syscall();
        }

        fn send_message_struct(ref self: ContractState, to_address: EthAddress, data: MyData) {
            let mut buf: Array<felt252> = array![];
            data.serialize(ref buf);
            starknet::send_message_to_l1_syscall(to_address.into(), buf.span()).unwrap_syscall();
        }
    }
}
