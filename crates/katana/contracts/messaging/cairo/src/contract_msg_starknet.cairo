//! A simple contract aims at being deployed on an appchain
//! to send/receive messages from Starknet.
//!
//! This contract can sends messages using the send message to l1
//! syscall as we normally do for messaging.
//!
//! However, the `to_address` is set to the `MSG` magic value since
//! this field is restricted to a valid Ethereum address, too small to
//! be a valid Starknet address.
use starknet::ContractAddress;

#[starknet::interface]
trait IContractAppchain<T> {
    /// Sends a message to Starknet contract with a single felt252 value.
    /// This message will simply be registered on starknet to be then consumed
    /// manually.
    ///
    /// # Arguments
    ///
    /// * `to_address` - Contract address on Starknet.
    /// * `value` - Value to be sent in the payload.
    fn send_message(ref self: T, to_address: ContractAddress, value: felt252);
}

#[starknet::contract]
mod contract_msg_starknet {
    use super::IContractAppchain;
    use starknet::{ContractAddress, SyscallResultTrait};

    #[storage]
    struct Storage {}

    /// Handles a message received from Starknet.
    ///
    /// Only functions that are #[l1_handler] can
    /// receive message from Starknet, exactly as we do with L1 messaging.
    ///
    /// # Arguments
    ///
    /// * `from_address` - The Starknet contract sending the message.
    /// * `value` - Expected value in the payload (automatically deserialized).
    #[l1_handler]
    fn msg_handler_value(ref self: ContractState, from_address: felt252, value: felt252) {
        // assert(from_address == ...);

        assert(value == 888, 'Invalid value');
    }

    #[abi(embed_v0)]
    impl ContractAppChainImpl of IContractAppchain<ContractState> {
        fn send_message(ref self: ContractState, to_address: ContractAddress, value: felt252) {
            let buf: Array<felt252> = array![to_address.into(), value];
            starknet::send_message_to_l1_syscall('MSG', buf.span()).unwrap_syscall();
        }
    }
}
