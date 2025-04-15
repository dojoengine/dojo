//! The messaging between an appchain and starknet
//! is done in a smiliar way starknet interacts with ethereum.
//!
//! This contract, deployed on starknet, will emit events.
//! An the sequencer of the appchain (katana in that case) will
//! listen for those events. When an event with a message is gathered
//! by katana, a L1 handler transaction is then created and added to the pool.
//!

/// Trait for Appchain messaging. For now, the messaging only whitelist one
/// appchain.
#[starknet::interface]
trait IAppchainMessaging<T> {
    /// Update the account address (on starknet or any chain where this contract is
    /// deployed) to accept messages.
    fn update_appchain_account_address(ref self: T, appchain_address: starknet::ContractAddress);

    /// Sends a message to an appchain by emitting an event.
    /// Returns the message hash and the nonce.
    fn send_message_to_appchain(
        ref self: T,
        to_address: starknet::ContractAddress,
        selector: felt252,
        payload: Span<felt252>,
    ) -> (felt252, felt252);

    /// Registers messages hashes as consumable.
    /// Usually, this function is only callable by the appchain developer/owner
    /// that control the appchain sequencer.
    fn add_messages_hashes_from_appchain(ref self: T, messages_hashes: Span<felt252>);

    /// Consumes a message registered as consumable by the appchain.
    /// This is the traditional consuming as done on ethereum.
    /// Returnes the message hash on success.
    fn consume_message_from_appchain(
        ref self: T, from_address: starknet::ContractAddress, payload: Span<felt252>,
    ) -> felt252;
}

#[starknet::interface]
trait IUpgradeable<T> {
    fn upgrade(ref self: T, class_hash: starknet::ClassHash);
}

#[starknet::contract]
mod appchain_messaging {
    use starknet::{ContractAddress, ClassHash};
    use starknet::storage::Map;
    use debug::PrintTrait;

    use super::{IAppchainMessaging, IUpgradeable};

    #[storage]
    struct Storage {
        // Owner of this contract.
        owner: ContractAddress,
        // The account on Starknet (or the chain where this contract is deployed)
        // used by the appchain sequencer to register messages hashes.
        appchain_account: ContractAddress,
        // The nonce for messages sent from Starknet.
        sn_to_appc_nonce: felt252,
        // Ledger of messages hashes sent from Starknet to the appchain.
        sn_to_appc_messages: Map::<felt252, felt252>,
        // Ledger of messages hashes registered from the appchain and a refcount
        // associated to it.
        appc_to_sn_messages: Map::<felt252, felt252>,
    }

    #[event]
    #[derive(Drop, starknet::Event)]
    enum Event {
        MessageSentToAppchain: MessageSentToAppchain,
        MessagesRegisteredFromAppchain: MessagesRegisteredFromAppchain,
        MessageConsumed: MessageConsumed,
        Upgraded: Upgraded,
    }

    #[derive(Drop, starknet::Event)]
    struct MessageSentToAppchain {
        #[key]
        message_hash: felt252,
        #[key]
        from: ContractAddress,
        #[key]
        to: ContractAddress,
        selector: felt252,
        nonce: felt252,
        payload: Span<felt252>,
    }

    #[derive(Drop, starknet::Event)]
    struct MessagesRegisteredFromAppchain {
        messages_hashes: Span<felt252>,
    }

    #[derive(Drop, starknet::Event)]
    struct MessageConsumed {
        #[key]
        message_hash: felt252,
        #[key]
        from: ContractAddress,
        #[key]
        to: ContractAddress,
        payload: Span<felt252>,
    }

    #[derive(Drop, starknet::Event)]
    struct Upgraded {
        class_hash: ClassHash,
    }

    #[constructor]
    fn constructor(
        ref self: ContractState, owner: ContractAddress, appchain_account: ContractAddress,
    ) {
        self.owner.write(owner);
        self.appchain_account.write(appchain_account);
    }

    /// Computes the starknet keccak to have a hash that fits in one felt.
    fn starknet_keccak(data: Span<felt252>) -> felt252 {
        let mut u256_data: Array<u256> = array![];

        let mut i = 0_usize;
        loop {
            if i == data.len() {
                break;
            }
            u256_data.append((*data[i]).into());
            i += 1;
        };

        let mut hash = keccak::keccak_u256s_be_inputs(u256_data.span());
        let low = integer::u128_byte_reverse(hash.high);
        let high = integer::u128_byte_reverse(hash.low);
        hash = u256 { low, high };
        hash = hash & 0x03ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff_u256;
        hash.try_into().expect('starknet keccak overflow')
    }

    /// Computes the hash of a message that is sent from the Appchain to Starknet.
    ///
    /// <https://github.com/starkware-libs/cairo-lang/blob/caba294d82eeeccc3d86a158adb8ba209bf2d8fc/src/starkware/starknet/solidity/StarknetMessaging.sol#L137>
    ///
    /// # Arguments
    ///
    /// * `from_address` - Contract address of the message sender on the Appchain.
    /// * `to_address` - Contract address to send the message to on the Appchain.
    /// * `payload` - The message payload.
    ///
    /// # Returns
    ///
    /// The hash of the message from the Appchain to Starknet.
    fn compute_hash_appc_to_sn(
        from_address: ContractAddress, to_address: ContractAddress, payload: Span<felt252>,
    ) -> felt252 {
        let mut hash_data: Array<felt252> = array![
            from_address.into(), to_address.into(), payload.len().into(),
        ];

        let mut i = 0_usize;
        loop {
            if i == payload.len() {
                break;
            }
            hash_data.append((*payload[i]));
            i += 1;
        };

        starknet_keccak(hash_data.span())
    }

    /// Computes the hash of a message that is sent from Starknet to the Appchain.
    ///
    /// <https://github.com/starkware-libs/cairo-lang/blob/caba294d82eeeccc3d86a158adb8ba209bf2d8fc/src/starkware/starknet/solidity/StarknetMessaging.sol#L88>
    ///
    /// # Arguments
    ///
    /// * `from_address` - Contract address of the message sender on the Appchain.
    /// * `to_address` - Contract address to send the message to on the Appchain.
    /// * `selector` - The `l1_handler` function selector of the contract on the Appchain
    ///                to execute.
    /// * `payload` - The message payload.
    /// * `nonce` - Nonce of the message.
    ///
    /// # Returns
    ///
    /// The hash of the message from Starknet to the Appchain.
    fn compute_hash_sn_to_appc(
        from_address: ContractAddress,
        to_address: ContractAddress,
        selector: felt252,
        payload: Span<felt252>,
        nonce: felt252,
    ) -> felt252 {
        let mut hash_data = array![
            from_address.into(), to_address.into(), nonce, selector, payload.len().into(),
        ];

        let mut i = 0_usize;
        loop {
            if i == payload.len() {
                break;
            }
            hash_data.append((*payload[i]));
            i += 1;
        };

        core::poseidon::poseidon_hash_span(hash_data.span())
    }

    #[abi(embed_v0)]
    impl AppchainMessagingUpgradeImpl of IUpgradeable<ContractState> {
        fn upgrade(ref self: ContractState, class_hash: ClassHash) {
            assert(
                starknet::get_caller_address() == self.owner.read(), 'Unauthorized replace class',
            );

            match starknet::replace_class_syscall(class_hash) {
                Result::Ok(_) => self.emit(Upgraded { class_hash }),
                Result::Err(revert_reason) => panic(revert_reason),
            };
        }
    }

    #[abi(embed_v0)]
    impl AppchainMessagingImpl of IAppchainMessaging<ContractState> {
        fn update_appchain_account_address(
            ref self: ContractState, appchain_address: ContractAddress,
        ) {
            assert(starknet::get_caller_address() == self.owner.read(), 'Unauthorized update');

            self.appchain_account.write(appchain_address);
        }

        fn send_message_to_appchain(
            ref self: ContractState,
            to_address: ContractAddress,
            selector: felt252,
            payload: Span<felt252>,
        ) -> (felt252, felt252) {
            let from_address = starknet::get_caller_address();
            let nonce = self.sn_to_appc_nonce.read() + 1;
            self.sn_to_appc_nonce.write(nonce);

            let msg_hash = compute_hash_sn_to_appc(
                from_address, to_address, selector, payload, nonce,
            );

            self
                .emit(
                    MessageSentToAppchain {
                        message_hash: msg_hash,
                        from: starknet::get_caller_address(),
                        to: to_address,
                        selector,
                        nonce,
                        payload,
                    },
                );

            self.sn_to_appc_messages.write(msg_hash, nonce);
            (msg_hash, nonce)
        }

        fn add_messages_hashes_from_appchain(
            ref self: ContractState, messages_hashes: Span<felt252>,
        ) {
            assert(
                self.appchain_account.read() == starknet::get_caller_address(),
                'Unauthorized hashes registrar',
            );

            let mut i = 0_usize;
            loop {
                if i == messages_hashes.len() {
                    break;
                }

                let msg_hash = *messages_hashes[i];

                let count = self.appc_to_sn_messages.read(msg_hash);
                self.appc_to_sn_messages.write(msg_hash, count + 1);

                i += 1;
            };

            self.emit(MessagesRegisteredFromAppchain { messages_hashes });
        }

        fn consume_message_from_appchain(
            ref self: ContractState, from_address: ContractAddress, payload: Span<felt252>,
        ) -> felt252 {
            let to_address = starknet::get_caller_address();

            let msg_hash = compute_hash_appc_to_sn(from_address, to_address, payload);

            let count = self.appc_to_sn_messages.read(msg_hash);
            assert(count.is_non_zero(), 'INVALID_MESSAGE_TO_CONSUME');

            self
                .emit(
                    MessageConsumed {
                        message_hash: msg_hash, from: from_address, to: to_address, payload,
                    },
                );

            self.appc_to_sn_messages.write(msg_hash, count - 1);

            msg_hash
        }
    }
}
