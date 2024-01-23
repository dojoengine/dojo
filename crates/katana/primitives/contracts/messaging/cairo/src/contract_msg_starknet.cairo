//! A simple contract aims at being deployed on an appchain
//! to send/receive messages from Starknet.
//!
//! This contract can sends messages using the send message to l1
//! syscall as we normally do for messaging.
//!
//! If the message contains a `to_address` that is not zero, the message
//! hash will be sent to starknet to be registered.
//! If the `to_address` is zero, then the message will then fire a transaction
//! on the starknet to directly execute the message content.
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

    /// Executes a message on Starknet. When the Katana will see this message
    /// with `to_address` set to 0, an invoke transaction will be fired.
    /// So basically this function can invoke any contract on Starknet, the fees on starknet
    /// being paid by the sequencer. We can here imagine several scenarios. :)
    /// The invoke though is not directly done to the destination contract, but the
    /// app messaging contract that will forward the execution.
    ///
    /// # Arguments
    ///
    /// * `to_address` - Contract address on Starknet.
    /// * `selector` - Selector.
    /// * `value` - Value to be sent as argument to the contract being executed on starknet.
    fn execute_message(ref self: T, to_address: ContractAddress, selector: felt252, value: felt252);
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

    #[external(v0)]
    impl ContractAppChainImpl of IContractAppchain<ContractState> {
        fn send_message(ref self: ContractState, to_address: ContractAddress, value: felt252) {
            let buf: Array<felt252> = array![to_address.into(), value];
            starknet::send_message_to_l1_syscall('MSG', buf.span()).unwrap_syscall();
        }

        fn execute_message(
            ref self: ContractState, to_address: ContractAddress, selector: felt252, value: felt252,
        ) {
            let buf: Array<felt252> = array![to_address.into(), selector, value];
            starknet::send_message_to_l1_syscall('EXE', buf.span()).unwrap_syscall();
        }
    }
}
