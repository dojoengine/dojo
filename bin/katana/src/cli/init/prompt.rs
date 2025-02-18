use std::cell::RefCell;
use std::rc::Rc;
use std::str::FromStr;
use std::sync::Arc;

use anyhow::{Context, Result};
use inquire::validator::{ErrorMessage, Validation};
use inquire::{Confirm, CustomType, Select};
use katana_primitives::block::BlockNumber;
use katana_primitives::{ContractAddress, Felt};
use starknet::accounts::{ExecutionEncoding, SingleOwnerAccount};
use starknet::core::types::{BlockId, BlockTag};
use starknet::core::utils::{cairo_short_string_to_felt, parse_cairo_short_string};
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::{JsonRpcClient, Provider, Url};
use starknet::signers::{LocalWallet, SigningKey};
use tokio::runtime::Handle;

use super::{deployment, Outcome};
use crate::cli::init::deployment::DeploymentOutcome;
use crate::cli::init::slot::{self, PaymasterAccountArgs};

pub const CARTRIDGE_SN_SEPOLIA_PROVIDER: &str = "https://api.cartridge.gg/x/starknet/sepolia";

pub async fn prompt() -> Result<Outcome> {
    let chain_id = CustomType::<String>::new("Id")
    .with_help_message("This will be the id of your rollup chain.")
    // checks that the input is a valid ascii string.
    .with_parser(&|input| {
        if !input.is_empty() && input.is_ascii() {
            Ok(input.to_string())
        } else {
            Err(())
        }
    })
    .with_error_message("Must be valid ASCII characters")
    .prompt()?;

    #[derive(Debug, Clone, strum_macros::Display)]
    enum SettlementChainOpt {
        Sepolia,
        #[cfg(feature = "init-custom-settlement-chain")]
        Custom,
    }

    // Right now we only support settling on Starknet Sepolia because we're limited to what
    // network the Atlantic service could settle the proofs to. Supporting a custom
    // network here (eg local devnet) would require that the proving service we're using
    // be able to settle the proofs there.
    let network_opts = vec![
        SettlementChainOpt::Sepolia,
        #[cfg(feature = "init-custom-settlement-chain")]
        SettlementChainOpt::Custom,
    ];

    let network_type = Select::new("Settlement chain", network_opts)
        .with_help_message("This is the chain where the rollup will settle on.")
        .prompt()?;

    let settlement_url = match network_type {
        SettlementChainOpt::Sepolia => Url::parse(CARTRIDGE_SN_SEPOLIA_PROVIDER)?,

        // Useful for testing the program flow without having to run it against actual network.
        #[cfg(feature = "init-custom-settlement-chain")]
        SettlementChainOpt::Custom => CustomType::<Url>::new("Settlement RPC URL")
            .with_default(Url::parse("http://localhost:5050")?)
            .with_error_message("Please enter a valid URL")
            .prompt()?,
    };

    let l1_provider = Arc::new(JsonRpcClient::new(HttpTransport::new(settlement_url.clone())));

    let contract_exist_parser = &|input: &str| {
        let block_id = BlockId::Tag(BlockTag::Pending);
        let address = Felt::from_str(input).map_err(|_| ())?;
        let result = tokio::task::block_in_place(|| {
            Handle::current().block_on(l1_provider.clone().get_class_hash_at(block_id, address))
        });

        match result {
            Ok(..) => Ok(ContractAddress::from(address)),
            Err(..) => Err(()),
        }
    };

    let account_address = CustomType::<ContractAddress>::new("Account")
        .with_error_message("Please enter a valid account address")
        .with_parser(contract_exist_parser)
        .prompt()?;

    let private_key = CustomType::<Felt>::new("Private key")
        .with_formatter(&|input: Felt| format!("{input:#x}"))
        .prompt()?;

    let l1_chain_id = l1_provider.chain_id().await?;
    let account = SingleOwnerAccount::new(
        l1_provider.clone(),
        LocalWallet::from_signing_key(SigningKey::from_secret_scalar(private_key)),
        account_address.into(),
        l1_chain_id,
        ExecutionEncoding::New,
    );

    // The core settlement contract on L1c.
    // Prompt the user whether to deploy the settlement contract or not.
    let deployment_outcome =
        if Confirm::new("Deploy settlement contract?").with_default(true).prompt()? {
            let chain_id = cairo_short_string_to_felt(&chain_id)?;
            deployment::deploy_settlement_contract(account, chain_id).await?
        }
        // If denied, prompt the user for an already deployed contract.
        else {
            let address = CustomType::<ContractAddress>::new("Settlement contract")
                .with_parser(contract_exist_parser)
                .prompt()?;

            // Check that the settlement contract has been initialized with the correct program
            // info.
            let chain_id = cairo_short_string_to_felt(&chain_id)?;
            deployment::check_program_info(chain_id, address.into(), &l1_provider).await.context(
                "Invalid settlement contract. The contract might have been configured incorrectly.",
            )?;

            let block_number =
                CustomType::<BlockNumber>::new("Settlement contract deployment block")
                    .with_help_message("The block at which the settlement contract was deployed")
                    .prompt()?;

            DeploymentOutcome { contract_address: address, block_number }
        };

    // It's wrapped like this because the prompt validator requires captured variables to have
    // 'static lifetime.
    let slot_paymasters: Rc<RefCell<Vec<PaymasterAccountArgs>>> = Default::default();
    let mut paymaster_count = 1;

    // Prompt for slot paymaster accounts
    while Confirm::new("Add Slot paymaster account?").with_default(true).prompt()? {
        let pubkey_prompt_text = format!("Paymaster #{} public key", paymaster_count);
        let public_key = CustomType::<Felt>::new(&pubkey_prompt_text)
            .with_formatter(&|input: Felt| format!("{input:#x}"))
            .prompt()?;

        // Check if this public_key + salt combo already exists
        // This check is necessary to ensure that each paymaster account has a unique addresses
        // because the contract address is derived from the public key and salt. So, if
        // there multiple paymasters with the same public key and salt pair, then
        // the resultant contract address will be the same.
        let slot_paymasters_clone = slot_paymasters.clone();
        let unique_salt_validator = move |salt: &Felt| {
            let pred = |pm: &PaymasterAccountArgs| pm.public_key == public_key && pm.salt == *salt;
            let duplicate = slot_paymasters_clone.borrow().iter().any(pred);

            if !duplicate {
                Ok(Validation::Valid)
            } else {
                Ok(Validation::Invalid(ErrorMessage::Custom(
                    "Public key and salt combination already exists!".to_string(),
                )))
            }
        };

        let salt_prompt_text = format!("Paymaster #{} salt", paymaster_count);
        let salt = CustomType::<Felt>::new(&salt_prompt_text)
            .with_formatter(&|input: Felt| format!("{input:#x}"))
            .with_validator(unique_salt_validator)
            .with_default(Felt::ONE)
            .prompt()?;

        slot_paymasters.borrow_mut().push(slot::PaymasterAccountArgs { public_key, salt });
        paymaster_count += 1;
    }

    Ok(Outcome {
        id: chain_id,
        deployment_outcome,
        rpc_url: settlement_url,
        account: account_address,
        settlement_id: parse_cairo_short_string(&l1_chain_id)?,
        #[cfg(feature = "init-slot")]
        slot_paymasters: Some(Rc::unwrap_or_clone(slot_paymasters).take()),
    })
}
