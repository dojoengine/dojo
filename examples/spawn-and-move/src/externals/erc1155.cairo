// SPDX-License-Identifier: MIT
// Compatible with OpenZeppelin Contracts for Cairo ^0.20.0

#[starknet::contract]
mod ERC1155Token {
    use OwnableComponent::InternalTrait;
    use openzeppelin::access::ownable::OwnableComponent;
    use openzeppelin::introspection::src5::SRC5Component;
    use openzeppelin::token::erc1155::{ERC1155Component, ERC1155HooksEmptyImpl};
    use openzeppelin::upgrades::UpgradeableComponent;
    use openzeppelin::upgrades::interface::IUpgradeable;
    use starknet::{ClassHash, ContractAddress};
    use crate::externals::components::erc4906::ERC4906Component;

    component!(path: ERC1155Component, storage: erc1155, event: ERC1155Event);
    component!(path: SRC5Component, storage: src5, event: SRC5Event);
    component!(path: OwnableComponent, storage: ownable, event: OwnableEvent);
    component!(path: ERC4906Component, storage: erc4906, event: ERC4906Event);
    component!(path: UpgradeableComponent, storage: upgradeable, event: UpgradeableEvent);

    // External
    #[abi(embed_v0)]
    impl ERC1155MixinImpl = ERC1155Component::ERC1155MixinImpl<ContractState>;
    #[abi(embed_v0)]
    impl OwnableMixinImpl = OwnableComponent::OwnableMixinImpl<ContractState>;
    #[abi(embed_v0)]
    impl ERC4906MixinImpl = ERC4906Component::ERC4906Implementation<ContractState>;

    // Internal
    impl ERC1155InternalImpl = ERC1155Component::InternalImpl<ContractState>;
    impl OwnableInternalImpl = OwnableComponent::InternalImpl<ContractState>;
    impl UpgradeableInternalImpl = UpgradeableComponent::InternalImpl<ContractState>;

    #[storage]
    struct Storage {
        #[substorage(v0)]
        erc1155: ERC1155Component::Storage,
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
        ERC1155Event: ERC1155Component::Event,
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
    fn constructor(ref self: ContractState, owner: ContractAddress, base_uri: ByteArray) {
        self.ownable.initializer(owner);
        self.erc1155.initializer(base_uri);
    }

    /// This implementation is not secure, only for testing purposes and quick minting.
    #[generate_trait]
    #[abi(per_item)]
    impl ExternalImpl of ExternalTrait {
        #[external(v0)]
        fn token_uri(ref self: ContractState, token_id: u256) -> ByteArray {
            let seed = starknet::get_execution_info().block_info.block_number;
            format!(
                "data:application/json,{{ \"image\": \"https://api.dicebear.com/9.x/lorelei-neutral/png?seed={}\" }}",
                seed,
            )
        }

        #[external(v0)]
        fn mint(ref self: ContractState, token_id: u256, value: u256) {
            self
                .erc1155
                .update(
                    starknet::contract_address_const::<0x0>(),
                    starknet::get_caller_address(),
                    array![token_id].span(),
                    array![value].span(),
                );
            // Seems to not be supported by default dojo account.
        // self.erc1155.mint_with_acceptance_check(account, token_id, value, data);
        }

        #[external(v0)]
        fn transfer_from(
            ref self: ContractState,
            from: ContractAddress,
            to: ContractAddress,
            token_id: u256,
            value: u256,
        ) {
            self.erc1155.update(from, to, array![token_id].span(), array![value].span());
            // safe transfer from does not support default account since they dont implement
        // receiver.
        }

        #[external(v0)]
        fn batch_mint(ref self: ContractState, token_ids: Span<u256>, values: Span<u256>) {
            self
                .erc1155
                .update(
                    starknet::contract_address_const::<0x0>(),
                    starknet::get_caller_address(),
                    token_ids,
                    values,
                );
            // Seems to not be supported by default dojo account.
        // self.erc1155.batch_mint_with_acceptance_check(account, token_ids, values, data);
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

        // Optional: Batch update specific token IDs
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
