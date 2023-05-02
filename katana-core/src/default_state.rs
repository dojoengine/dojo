use starknet_api::{
    core::{ClassHash, ContractAddress, PatriciaKey},
    hash::{StarkFelt, StarkHash},
    patricia_key, stark_felt,
};

use crate::{
    constants::{
        ERC20_CONTRACT_CLASS_HASH, ERC20_CONTRACT_PATH, FEE_ERC20_CONTRACT_ADDRESS,
        UNIVERSAL_DEPLOYER_CLASS_HASH, UNIVERSAL_DEPLOYER_CONTRACT_ADDRESS,
        UNIVERSAL_DEPLOYER_CONTRACT_PATH,
    },
    state::DictStateReader,
    util::get_contract_class,
};

pub struct KatanaDefaultState;

impl KatanaDefaultState {
    pub fn initialize_state(state: &mut DictStateReader) {
        Self::deploy_fee_contract(state);
        Self::deploy_universal_deployer_contract(state);
    }

    fn deploy_fee_contract(state: &mut DictStateReader) {
        let erc20_class_hash = ClassHash(stark_felt!(ERC20_CONTRACT_CLASS_HASH));
        state
            .class_hash_to_class
            .insert(erc20_class_hash, get_contract_class(ERC20_CONTRACT_PATH));
        state.address_to_class_hash.insert(
            ContractAddress(patricia_key!(FEE_ERC20_CONTRACT_ADDRESS)),
            erc20_class_hash,
        );
    }

    fn deploy_universal_deployer_contract(state: &mut DictStateReader) {
        let universal_deployer_class_hash = ClassHash(stark_felt!(UNIVERSAL_DEPLOYER_CLASS_HASH));
        state.class_hash_to_class.insert(
            universal_deployer_class_hash,
            get_contract_class(UNIVERSAL_DEPLOYER_CONTRACT_PATH),
        );
        state.address_to_class_hash.insert(
            ContractAddress(patricia_key!(UNIVERSAL_DEPLOYER_CONTRACT_ADDRESS)),
            universal_deployer_class_hash,
        );
    }
}
