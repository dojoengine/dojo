//! Simple contract to send / consume message from appchain.

use starknet::ContractAddress;

#[starknet::interface]
trait Icontract_1<T> {
  fn send_message( ref self: T, to_address: ContractAddress, selector: felt252, value: felt252);
  fn consume_message(ref self: T, from_address: ContractAddress, payload: Span<felt252>);
  fn set_value(ref self: T, value: felt252);
  fn get_value(self: @T) -> felt252;
}

#[starknet::contract]
mod contract_1 {
    use super::Icontract_1;
    use starknet::ContractAddress;
    use katana_messaging::appchain_messaging::{
        IAppchainMessagingDispatcher, IAppchainMessagingDispatcherTrait,
    };

    #[storage]
    struct Storage {
        value: felt252,
        messaging_contract: ContractAddress,
    }

    #[constructor]
    fn constructor(ref self: ContractState, messaging_contract: ContractAddress,) {
        self.messaging_contract.write(messaging_contract);
    }

    #[abi(embed_v0)]
    impl Icontract_1Impl of Icontract_1<ContractState>{
    
        /// Sends a message with the given value.
        fn send_message(
            ref self: ContractState, to_address: ContractAddress, selector: felt252, value: felt252,
        ) {
            let messaging = IAppchainMessagingDispatcher {
                contract_address: self.messaging_contract.read()
            };

            messaging.send_message_to_appchain(to_address, selector, array![value].span(),);
        }

        /// Consume a message registered by the appchain.
        fn consume_message(
            ref self: ContractState, from_address: ContractAddress, payload: Span<felt252>,
        ) {
            let messaging = IAppchainMessagingDispatcher {
                contract_address: self.messaging_contract.read()
            };

            // Will revert in case of failure if the message is not registered
            // as consumable.
            let msg_hash = messaging.consume_message_from_appchain(from_address, payload,);
        // msg successfully consumed, we can proceed and process the data
        // in the payload.
        }

        /// An example function to test how appchain contract can trigger
        /// code execution on Starknet.
        fn set_value(ref self: ContractState, value: felt252) {
            self.value.write(value);
        }

        fn get_value(self: @ContractState) -> felt252 {
            self.value.read()
        }
    }
}
