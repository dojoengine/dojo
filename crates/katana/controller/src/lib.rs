use std::collections::HashMap;

use account_sdk::abigen::controller::{Signer, SignerType};
use account_sdk::signers::webauthn::{DeviceSigner, WebauthnAccountSigner};
use account_sdk::signers::SignerTrait;
use account_sdk::wasm_webauthn::CredentialID;
use alloy_primitives::U256;
use anyhow::Result;
use coset::CoseKey;
use katana_primitives::contract::{ContractAddress, StorageKey, StorageValue};
use katana_primitives::genesis::allocation::{GenesisAllocation, GenesisContractAlloc};
use katana_primitives::genesis::constant::CONTROLLER_ACCOUNT_CONTRACT_CLASS_HASH;
use katana_primitives::genesis::Genesis;
use katana_primitives::FieldElement;
use slot::credential::Credentials;
use starknet::core::utils::get_storage_var_address;
use tracing::trace;

mod webauthn;

const LOG_TARGET: &str = "katana::controller";

const WEBAUTHN_RP_ID: &str = "cartridge.gg";
const WEBAUTHN_ORIGIN: &str = "https://x.cartridge.gg";

pub fn add_controller_account(genesis: &mut Genesis) -> Result<()> {
    // bouncer that checks if there is an authenticated slot user
    let credentials = Credentials::load()?;
    add_controller_account_inner(genesis, credentials.account)
}

fn add_controller_account_inner(genesis: &mut Genesis, user: slot::account::Account) -> Result<()> {
    let cred = user.credentials.webauthn.first().unwrap();

    trace!(
        target: LOG_TARGET,
        username = user.id,
        address = format!("{:#x}", user.contract_address),
        "Adding Cartridge Controller account to genesis."
    );

    let credential_id = webauthn::credential::from_base64(&cred.id)?;
    let public_key = webauthn::cose_key::from_base64(&cred.public_key)?;

    let (address, contract) = {
        let account = GenesisContractAlloc {
            nonce: None,
            balance: Some(U256::from(0xfffffffffffffffu128)),
            class_hash: Some(CONTROLLER_ACCOUNT_CONTRACT_CLASS_HASH),
            storage: Some(get_contract_storage(credential_id, public_key, SignerType::Webauthn)?),
        };

        (ContractAddress::from(user.contract_address), GenesisAllocation::Contract(account))
    };

    genesis.extend_allocations([(address, contract)]);

    trace!(
        target: LOG_TARGET,
        username = user.id,
        address = format!("{:#x}", user.contract_address),
        "Cartridge Controller account added to genesis."
    );

    Ok(())
}

pub mod json {
    use anyhow::Result;
    use katana_primitives::genesis::json::{
        ClassNameOrHash, GenesisClassJson, GenesisContractJson, GenesisJson,
    };
    use serde_json::Value;

    use super::*;

    const CONTROLLER_SIERRA_ARTIFACT: &str =
        include_str!("../../contracts/compiled/controller_CartridgeAccount.contract_class.json");
    const CONTROLLER_CLASS_NAME: &str = "controller";

    // TODO(kariy): should accept the whole account struct instead of individual fields
    // build the genesis json file
    pub fn add_controller_account_json(genesis: &mut GenesisJson) -> Result<()> {
        // bouncer that checks if there is an authenticated slot user
        let user = Credentials::load()?;
        let cred = user.account.credentials.webauthn.first().unwrap();

        let credential_id = webauthn::credential::from_base64(&cred.id)?;
        let public_key = webauthn::cose_key::from_base64(&cred.public_key)?;

        add_controller_class_json(genesis)?;

        let (address, contract) = {
            let account = GenesisContractJson {
                nonce: None,
                balance: None,
                class: Some(ClassNameOrHash::Name(CONTROLLER_CLASS_NAME.to_string())),
                storage: Some(get_contract_storage(
                    credential_id,
                    public_key,
                    SignerType::Webauthn,
                )?),
            };

            (ContractAddress::from(user.account.contract_address), account)
        };

        genesis.contracts.insert(address, contract);

        Ok(())
    }

    fn add_controller_class_json(genesis: &mut GenesisJson) -> Result<()> {
        // parse the controller class json file
        let json = serde_json::from_str::<Value>(CONTROLLER_SIERRA_ARTIFACT)?;

        let class = GenesisClassJson {
            class_hash: None,
            class: json.into(),
            name: Some(CONTROLLER_CLASS_NAME.to_string()),
        };

        genesis.classes.push(class);

        Ok(())
    }
}

fn get_contract_storage(
    credential_id: CredentialID,
    public_key: CoseKey,
    signer_type: SignerType,
) -> Result<HashMap<StorageKey, StorageValue>> {
    let type_value: u16 = match signer_type {
        SignerType::Starknet => 0,
        SignerType::Secp256k1 => 1,
        SignerType::Webauthn => 4,
        SignerType::Unimplemented => 999,
    };

    let signer = DeviceSigner::new(
        WEBAUTHN_RP_ID.to_string(),
        WEBAUTHN_ORIGIN.to_string(),
        credential_id,
        public_key,
    );

    let signer = Signer::Webauthn(signer.signer_pub_data());
    let guid = signer.guid();

    // the storage variable name for webauthn signer
    const NON_STARK_OWNER_VAR_NAME: &str = "_owner_non_stark";
    let type_value = FieldElement::from(type_value);
    let storage = get_storage_var_address(NON_STARK_OWNER_VAR_NAME, &[type_value])?;

    Ok(HashMap::from([(storage, guid)]))
}

#[cfg(test)]
mod tests {
    use slot::account::WebAuthnCredential;
    use starknet::macros::felt;

    use super::*;

    // Test data for Controller with WebAuthn Signer.
    //
    // Username: johnsmith
    // Controller address: 0x0260ab0352da372054ed9dc586f024f6a259b9ea64a8e09b16147201220f88d2
    // <https://sepolia.starkscan.co/contract/0x0260ab0352da372054ed9dc586f024f6a259b9ea64a8e09b16147201220f88d2#overview>

    const STORAGE_KEY: FieldElement =
        felt!("0x058c7ee1e9bb09b0d728314f36629772ef7a3c6773a823064d5a7e5651bcb890");
    const STORAGE_VALUE: FieldElement =
        felt!("0x5d7709b0a485e64a549ada9bd14d30419364127dfd351e01f38871c82500cd7");

    const WEBAUTHN_CREDENTIAL_ID: &str = "ja0NkHny-dlfPnClYECdmce0xTCuGT0xFjeuStaVqCI";
    const WEBAUTHN_PUBLIC_KEY: &str = "pQECAyYgASFYIBLHWNmpxCtO47cfOXw9nFCGftMq57xhvQC98aY_zQchIlggIgGHmWwQe1_FGi9GYqcYYpoPC9mkkf0f1rVD5UoGPEA";

    #[test]
    fn test_add_controller_account() {
        let mut genesis = Genesis::default();

        let account = slot::account::Account {
            id: "johnsmith".to_string(),
            name: None,
            contract_address: felt!("1337"),
            credentials: slot::account::AccountCredentials {
                webauthn: vec![WebAuthnCredential {
                    id: WEBAUTHN_CREDENTIAL_ID.to_string(),
                    public_key: WEBAUTHN_PUBLIC_KEY.to_string(),
                }],
            },
        };

        add_controller_account_inner(&mut genesis, account.clone()).unwrap();

        let address = ContractAddress::from(account.contract_address);
        let allocation = genesis.allocations.get(&address).unwrap();

        assert!(genesis.allocations.contains_key(&address));
        assert_eq!(allocation.balance(), Some(U256::from(0xfffffffffffffffu128)));
        assert_eq!(allocation.class_hash(), Some(CONTROLLER_ACCOUNT_CONTRACT_CLASS_HASH));
    }

    #[test]
    fn test_get_contract_storage() {
        let credential_id = webauthn::credential::from_base64(WEBAUTHN_CREDENTIAL_ID).unwrap();
        let public_key = webauthn::cose_key::from_base64(WEBAUTHN_PUBLIC_KEY).unwrap();

        let storage =
            get_contract_storage(credential_id.clone(), public_key.clone(), SignerType::Webauthn)
                .unwrap();

        assert_eq!(storage.len(), 1);
        assert_eq!(storage.get(&STORAGE_KEY), Some(&STORAGE_VALUE));
    }
}
