// SPDX-License-Identifier: MIT
// Compatible with OpenZeppelin Contracts for Cairo ^0.20.0

#[starknet::contract]
mod ERC721Token {
    use OwnableComponent::InternalTrait;
    use openzeppelin::access::ownable::OwnableComponent;
    use openzeppelin::introspection::src5::SRC5Component;
    use openzeppelin::token::erc721::{ERC721Component, ERC721HooksEmptyImpl};
    use openzeppelin::upgrades::UpgradeableComponent;
    use openzeppelin::upgrades::interface::IUpgradeable;
    use starknet::{ClassHash, ContractAddress};
    use crate::externals::components::erc4906::ERC4906Component;

    component!(path: ERC721Component, storage: erc721, event: ERC721Event);
    component!(path: SRC5Component, storage: src5, event: SRC5Event);
    component!(path: OwnableComponent, storage: ownable, event: OwnableEvent);
    component!(path: ERC4906Component, storage: erc4906, event: ERC4906Event);
    component!(path: UpgradeableComponent, storage: upgradeable, event: UpgradeableEvent);

    // External
    #[abi(embed_v0)]
    impl ERC721MixinImpl = ERC721Component::ERC721MixinImpl<ContractState>;
    #[abi(embed_v0)]
    impl OwnableMixinImpl = OwnableComponent::OwnableMixinImpl<ContractState>;
    #[abi(embed_v0)]
    impl ERC4906MixinImpl = ERC4906Component::ERC4906Implementation<ContractState>;

    // Internal
    impl ERC721InternalImpl = ERC721Component::InternalImpl<ContractState>;
    impl OwnableInternalImpl = OwnableComponent::InternalImpl<ContractState>;
    impl UpgradeableInternalImpl = UpgradeableComponent::InternalImpl<ContractState>;

    #[storage]
    struct Storage {
        #[substorage(v0)]
        erc721: ERC721Component::Storage,
        #[substorage(v0)]
        src5: SRC5Component::Storage,
        #[substorage(v0)]
        ownable: OwnableComponent::Storage,
        #[substorage(v0)]
        erc4906: ERC4906Component::Storage,
        #[substorage(v0)]
        upgradeable: UpgradeableComponent::Storage,
    }

    #[event]
    #[derive(Drop, starknet::Event)]
    enum Event {
        #[flat]
        ERC721Event: ERC721Component::Event,
        #[flat]
        SRC5Event: SRC5Component::Event,
        #[flat]
        OwnableEvent: OwnableComponent::Event,
        #[flat]
        ERC4906Event: ERC4906Component::Event,
        #[flat]
        UpgradeableEvent: UpgradeableComponent::Event,
    }

    #[constructor]
    fn constructor(
        ref self: ContractState,
        owner: ContractAddress,
        name: ByteArray,
        symbol: ByteArray,
        base_uri: ByteArray,
    ) {
        self.ownable.initializer(owner);
        self.erc721.initializer(name, symbol, base_uri);
    }

    /// This implementation is not secure, only for testing purposes and quick minting.
    #[generate_trait]
    #[abi(per_item)]
    impl ERC721Demo of ERC721DemoTrait {
        #[external(v0)]
        fn mint(ref self: ContractState, token_id: u256) {
            self.erc721.mint(starknet::get_caller_address(), token_id);
        }

        #[external(v0)]
        fn update_token_metadata(ref self: ContractState, token_id: u256) {
            // Only owner can update metadata
            self.ownable.assert_only_owner();

            // Emit metadata update event
            self.erc4906.emit_metadata_update(token_id);
        }

        #[external(v0)]
        fn update_batch_token_metadata(
            ref self: ContractState, from_token_id: u256, to_token_id: u256,
        ) {
            // Only owner can update metadata
            self.ownable.assert_only_owner();

            // Emit batch metadata update event
            self.erc4906.emit_batch_metadata_update(from_token_id, to_token_id);
        }

        #[external(v0)]
        fn update_tokens_metadata(ref self: ContractState, token_ids: Span<u256>) {
            // Only owner can update metadata
            self.ownable.assert_only_owner();

            // Emit metadata update event for each token
            let mut i: usize = 0;
            loop {
                if i >= token_ids.len() {
                    break;
                }
                self.erc4906.emit_metadata_update(*token_ids.at(i));
                i += 1;
            }
        }
    }

    #[abi(embed_v0)]
    impl UpgradeableImpl of IUpgradeable<ContractState> {
        fn upgrade(ref self: ContractState, new_class_hash: ClassHash) {
            self.ownable.assert_only_owner();
            self.upgradeable.upgrade(new_class_hash);
        }
    }
}
