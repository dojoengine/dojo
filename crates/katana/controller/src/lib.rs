use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

use account_sdk::abigen::controller::{Signer, SignerType};
use account_sdk::signers::webauthn::{DeviceSigner, WebauthnAccountSigner};
use account_sdk::signers::SignerTrait;
use account_sdk::wasm_webauthn::CredentialID;
use alloy_primitives::U256;
use anyhow::Result;
use coset::CoseKey;
use katana_primitives::class::{ClassHash, CompiledClass, SierraCompiledClass};
use katana_primitives::contract::{ContractAddress, StorageKey, StorageValue};
use katana_primitives::genesis::allocation::{GenesisAllocation, GenesisContractAlloc};
use katana_primitives::genesis::{Genesis, GenesisClass};
use katana_primitives::utils::class::{parse_compiled_class_v1, parse_sierra_class};
use katana_primitives::FieldElement;
use slot::credential::Credentials;
use slot::graphql::auth::me::MeMe;
use starknet::core::utils::get_storage_var_address;

mod webauthn;

const CONTROLLER_SIERRA_ARTIFACT: &str =
    include_str!("../../contracts/compiled/controller_CartridgeAccount.contract_class.json");

const WEBAUTHN_RP_ID: &str = "cartridge.gg";
const WEBAUTHN_ORIGIN: &str = "https://x.cartridge.gg";

fn add_controller_class(genesis: &mut Genesis) -> Result<ClassHash> {
    let sierra = parse_sierra_class(CONTROLLER_SIERRA_ARTIFACT)?;
    let class_hash = sierra.class_hash()?;
    let sierra = sierra.flatten()?;
    let casm = read_compiled_class_artifact(CONTROLLER_SIERRA_ARTIFACT)?;
    let casm_hash = FieldElement::from_bytes_be(&casm.casm.compiled_class_hash().to_be_bytes())?;

    genesis.classes.insert(
        class_hash,
        GenesisClass {
            sierra: Some(Arc::new(sierra)),
            compiled_class_hash: casm_hash,
            casm: Arc::new(CompiledClass::Class(casm)),
        },
    );

    Ok(class_hash)
}

// TODO(kariy): should accept the whole account struct instead of individual fields
// build the genesis file
pub fn add_controller_account(genesis: &mut Genesis) -> Result<()> {
    // bouncer that checks if there is an authenticated slot user
    let user = Credentials::load()?;

    let MeMe { credentials, contract_address, .. } = user.account.unwrap();

    let address = FieldElement::from_str(&contract_address.unwrap())?;
    let creds = credentials.webauthn.unwrap();
    let cred = creds.first().unwrap();

    let class_hash = add_controller_class(genesis)?;

    let credential_id = webauthn::credential::from_base64(&cred.id)?;
    let public_key = webauthn::cose_key::from_base64(&cred.public_key)?;

    let (address, contract) = {
        let account = GenesisContractAlloc {
            nonce: None,
            class_hash: Some(class_hash),
            balance: Some(U256::from(0xfffffffffffffffu128)),
            storage: Some(get_contract_storage(credential_id, public_key, SignerType::Webauthn)?),
        };

        (ContractAddress::from(address), GenesisAllocation::Contract(account))
    };

    genesis.extend_allocations([(address, contract)]);
    Ok(())
}

pub mod json {
    use anyhow::Result;
    use katana_primitives::genesis::json::{
        ClassNameOrHash, GenesisClassJson, GenesisContractJson, GenesisJson,
    };
    use serde_json::Value;

    use super::*;

    const CONTROLLER_CLASS_NAME: &str = "controller";

    // TODO(kariy): should accept the whole account struct instead of individual fields
    // build the genesis json file
    pub fn add_controller_account_json(genesis: &mut GenesisJson) -> Result<()> {
        // bouncer that checks if there is an authenticated slot user
        let user = Credentials::load()?;

        let MeMe { credentials, contract_address, .. } = user.account.unwrap();

        let address = FieldElement::from_str(&contract_address.unwrap())?;
        let creds = credentials.webauthn.unwrap();
        let cred = creds.first().unwrap();

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

            (ContractAddress::from(address), account)
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

fn read_compiled_class_artifact(artifact: &str) -> Result<SierraCompiledClass> {
    let value = serde_json::from_str(artifact)?;
    parse_compiled_class_v1(value)
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_add_controller_account() {}
}
