use std::collections::BTreeMap;

use alloy_primitives::U256;
use anyhow::Result;
use async_trait::async_trait;
use coset::CoseKey;
use katana_primitives::contract::{ContractAddress, StorageKey, StorageValue};
use katana_primitives::genesis::allocation::{GenesisAllocation, GenesisContractAlloc};
use katana_primitives::genesis::constant::CONTROLLER_CLASS_HASH;
use katana_primitives::genesis::Genesis;
use katana_primitives::Felt;
use slot::account_sdk::signers::webauthn::{CredentialID, WebauthnBackend, WebauthnSigner};
use slot::account_sdk::signers::{HashSigner, Signer, SignerTrait};
use slot::account_sdk::OriginProvider;
use slot::credential::Credentials;
use starknet::core::utils::get_storage_var_address;
use tracing::trace;

mod webauthn;

const WEBAUTHN_RP_ID: &str = "cartridge.gg";
const WEBAUTHN_ORIGIN: &str = "https://x.cartridge.gg";

pub fn add_controller_account(genesis: &mut Genesis) -> Result<()> {
    // bouncer that checks if there is an authenticated slot user
    let credentials = Credentials::load()?;
    add_controller_account_inner(genesis, credentials.account)
}

fn add_controller_account_inner(
    genesis: &mut Genesis,
    user: slot::account::AccountInfo,
) -> Result<()> {
    let cred = user.credentials.first().unwrap();
    let contract_address = user.controllers.first().unwrap().address;

    trace!(
        username = user.id,
        address = format!("{:#x}", contract_address),
        "Adding Cartridge Controller account to genesis."
    );

    let credential_id = webauthn::credential::from_base64(&cred.id)?;
    let public_key = webauthn::cose_key::from_base64(&cred.public_key)?;

    let (address, contract) = {
        let account = GenesisContractAlloc {
            nonce: None,
            balance: Some(U256::from(0xfffffffffffffffu128)),
            class_hash: Some(CONTROLLER_CLASS_HASH),
            storage: Some(get_contract_storage(credential_id, public_key)?),
        };

        let address = ContractAddress::from(contract_address);

        (address, GenesisAllocation::Contract(account))
    };

    genesis.extend_allocations([(address, contract)]);

    trace!(
        username = user.id,
        address = format!("{:#x}", contract_address),
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
        include_str!("../../contracts/build/controller_CartridgeAccount.contract_class.json");
    const CONTROLLER_CLASS_NAME: &str = "controller";

    // TODO(kariy): should accept the whole account struct instead of individual fields
    // build the genesis json file
    pub fn add_controller_account_json(genesis: &mut GenesisJson) -> Result<()> {
        // bouncer that checks if there is an authenticated slot user
        let user = Credentials::load()?;
        let cred = user.account.credentials.first().unwrap();
        let contract_address = user.account.controllers.first().unwrap().address;

        let credential_id = webauthn::credential::from_base64(&cred.id)?;
        let public_key = webauthn::cose_key::from_base64(&cred.public_key)?;

        add_controller_class_json(genesis)?;

        let (address, contract) = {
            let account = GenesisContractJson {
                nonce: None,
                balance: None,
                class: Some(ClassNameOrHash::Name(CONTROLLER_CLASS_NAME.to_string())),
                storage: Some(get_contract_storage(credential_id, public_key)?),
            };

            let address = ContractAddress::from(contract_address);

            (address, account)
        };

        genesis.contracts.insert(address, contract);

        Ok(())
    }

    fn add_controller_class_json(genesis: &mut GenesisJson) -> Result<()> {
        // parse the controller class json file
        let json = serde_json::from_str::<Value>(CONTROLLER_SIERRA_ARTIFACT)?;

        let class =
            GenesisClassJson { class: json.into(), name: Some(CONTROLLER_CLASS_NAME.to_string()) };

        genesis.classes.push(class);

        Ok(())
    }
}

/// Get the constructor contract storage for the Controller account
///
/// Reference: https://github.com/cartridge-gg/controller/blob/e76107998c33344d93304012fa6ff13f4003828a/packages/contracts/controller/src/account.cairo#L217
fn get_contract_storage(
    credential_id: CredentialID,
    public_key: CoseKey,
) -> Result<BTreeMap<StorageKey, StorageValue>> {
    use slot::account_sdk::signers::DeviceError;
    use webauthn_rs_proto::auth::PublicKeyCredentialRequestOptions;
    use webauthn_rs_proto::{
        PublicKeyCredential, PublicKeyCredentialCreationOptions, RegisterPublicKeyCredential,
    };

    // Our main priority here is to just compute the guid ( which is the call to `guid()` below )
    //
    // Technically we dont need to implement all of these and can just compute the guid directly
    // ourselves, but this way its much simpler and easier to understand what is going on and also
    // less prone to error ( assuming the underlying implementation is correct ).

    #[derive(Debug)]
    struct SlotBackend;

    impl OriginProvider for SlotBackend {
        fn origin(&self) -> Result<String, DeviceError> {
            Ok(WEBAUTHN_ORIGIN.to_string())
        }
    }

    // SAFETY:
    //
    // We don't be calling any of these functions throughout the process of computing the
    // guid, so it's safe to just return an error here.

    #[async_trait]
    impl WebauthnBackend for SlotBackend {
        async fn get_assertion(
            &self,
            options: PublicKeyCredentialRequestOptions,
        ) -> Result<PublicKeyCredential, DeviceError> {
            let _ = options;
            Err(DeviceError::GetAssertion("Not implemented".to_string()))
        }

        async fn create_credential(
            &self,
            options: PublicKeyCredentialCreationOptions,
        ) -> Result<RegisterPublicKeyCredential, DeviceError> {
            let _ = options;
            Err(DeviceError::CreateCredential("Not implemented".to_string()))
        }
    }

    let webauthn_signer =
        WebauthnSigner::new(WEBAUTHN_RP_ID.to_string(), credential_id, public_key, SlotBackend);
    let guid = Signer::Webauthn(webauthn_signer).signer().guid();

    // the storage variable name in the Controller contract for storing owners' credentials
    const MULTIPLE_OWNERS_COMPONENT_SUB_STORAGE: &str = "owners";
    let storage = get_storage_var_address(MULTIPLE_OWNERS_COMPONENT_SUB_STORAGE, &[guid])?;

    // 1 for boolean True in Cairo. Refer to the provided link above.
    let storage_mapping = BTreeMap::from([(storage, Felt::ONE)]);

    Ok(storage_mapping)
}

#[cfg(test)]
mod tests {

    use assert_matches::assert_matches;
    use slot::account::{Controller, ControllerSigner, SignerType, WebAuthnCredential};
    use starknet::macros::felt;

    use super::*;

    // Test data for Controller with WebAuthn Signer.
    //
    // Username: johnsmith
    // Controller address: 0x00397333e993ae162b476690e1401548ae97a8819955506b8bc918e067bdafc3
    // <https://sepolia.starkscan.co/contract/0x00397333e993ae162b476690e1401548ae97a8819955506b8bc918e067bdafc3#contract-storage>

    const CONTROLLER_ADDRESS: Felt =
        felt!("0x00397333e993ae162b476690e1401548ae97a8819955506b8bc918e067bdafc3");

    const STORAGE_KEY: Felt =
        felt!("0x023d8ecd0d641047a8d21e3cd8016377ed5c9cd9009539cd92b73adb8c023f10");
    const STORAGE_VALUE: Felt = felt!("0x1");

    const WEBAUTHN_CREDENTIAL_ID: &str = "ja0NkHny-dlfPnClYECdmce0xTCuGT0xFjeuStaVqCI";
    const WEBAUTHN_PUBLIC_KEY: &str = "pQECAyYgASFYIBLHWNmpxCtO47cfOXw9nFCGftMq57xhvQC98aY_zQchIlggIgGHmWwQe1_FGi9GYqcYYpoPC9mkkf0f1rVD5UoGPEA";

    #[test]
    fn test_add_controller_account() {
        let mut genesis = Genesis::default();

        let account = slot::account::AccountInfo {
            id: "johnsmith".to_string(),
            name: None,
            controllers: vec![Controller {
                id: "controller1".to_string(),
                address: CONTROLLER_ADDRESS,
                signers: vec![ControllerSigner {
                    id: "signer1".to_string(),
                    r#type: SignerType::WebAuthn,
                }],
            }],
            credentials: vec![WebAuthnCredential {
                id: WEBAUTHN_CREDENTIAL_ID.to_string(),
                public_key: WEBAUTHN_PUBLIC_KEY.to_string(),
            }],
        };

        add_controller_account_inner(&mut genesis, account.clone()).unwrap();

        let address = ContractAddress::from(account.controllers[0].address);
        let allocation = genesis.allocations.get(&address).unwrap();

        assert!(genesis.allocations.contains_key(&address));
        assert_eq!(allocation.balance(), Some(U256::from(0xfffffffffffffffu128)));
        assert_eq!(allocation.class_hash(), Some(CONTROLLER_CLASS_HASH));

        // Check the owner storage value
        assert_matches!(allocation, GenesisAllocation::Contract(contract) => {
            let storage = contract.storage.as_ref().unwrap();
            assert_eq!(storage.get(&STORAGE_KEY), Some(&STORAGE_VALUE));
        });
    }

    #[test]
    fn test_get_contract_storage() {
        let credential_id = webauthn::credential::from_base64(WEBAUTHN_CREDENTIAL_ID).unwrap();
        let public_key = webauthn::cose_key::from_base64(WEBAUTHN_PUBLIC_KEY).unwrap();

        let storage = get_contract_storage(credential_id.clone(), public_key.clone()).unwrap();

        assert_eq!(storage.len(), 1);
        assert_eq!(storage.get(&STORAGE_KEY), Some(&STORAGE_VALUE));
    }
}
