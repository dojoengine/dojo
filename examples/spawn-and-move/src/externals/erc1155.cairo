// SPDX-License-Identifier: MIT
// Compatible with OpenZeppelin Contracts for Cairo ^0.20.0

#[starknet::contract]
mod ERC1155Token {
    use openzeppelin::access::ownable::OwnableComponent;
    use openzeppelin::introspection::src5::SRC5Component;
    use openzeppelin::token::erc1155::{ERC1155Component, ERC1155HooksEmptyImpl};
    use starknet::ContractAddress;

    component!(path: ERC1155Component, storage: erc1155, event: ERC1155Event);
    component!(path: SRC5Component, storage: src5, event: SRC5Event);
    component!(path: OwnableComponent, storage: ownable, event: OwnableEvent);

    // External
    #[abi(embed_v0)]
    impl ERC1155MixinImpl = ERC1155Component::ERC1155MixinImpl<ContractState>;
    #[abi(embed_v0)]
    impl OwnableMixinImpl = OwnableComponent::OwnableMixinImpl<ContractState>;

    // Internal
    impl ERC1155InternalImpl = ERC1155Component::InternalImpl<ContractState>;
    impl OwnableInternalImpl = OwnableComponent::InternalImpl<ContractState>;

    #[storage]
    struct Storage {
        #[substorage(v0)]
        erc1155: ERC1155Component::Storage,
        #[substorage(v0)]
        src5: SRC5Component::Storage,
        #[substorage(v0)]
        ownable: OwnableComponent::Storage,
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
    }
}
