use anyhow::{Context, Result};
use clap::Args;
use dojo_utils::{Deployer, TransactionResult, TxnConfig};
use dojo_world::config::calldata_decoder::decode_calldata;
use sozo_ui::SozoUi;
use starknet::core::types::Felt;
use starknet::core::utils::get_contract_address;
use tracing::trace;

use super::options::account::AccountOptions;
use super::options::starknet::StarknetOptions;
use super::options::transaction::TransactionOptions;
use crate::utils::{get_account_from_env, CALLDATA_DOC};

#[derive(Debug, Args)]
#[command(about = "Deploy a declared class through the Universal Deployer Contract (UDC).")]
pub struct DeployArgs {
    #[arg(value_name = "CLASS_HASH", help = "The class hash to deploy.")]
    pub class_hash: Felt,

    #[arg(long, default_value = "0x0", help = "Salt to use for the deployment.")]
    pub salt: Felt,

    #[arg(
        long,
        default_value = "0x0",
        help = "Deployer address to pass to the UDC. Defaults to zero for standard deployments."
    )]
    pub deployer_address: Felt,

    #[arg(
        long = "constructor-calldata",
        value_name = "ARG",
        num_args = 0..,
        help = format!(
            "Constructor calldata elements (space separated).\n\n{}",
            CALLDATA_DOC
        )
    )]
    pub constructor_calldata: Vec<String>,

    #[command(flatten)]
    pub transaction: TransactionOptions,

    #[command(flatten)]
    pub starknet: StarknetOptions,

    #[command(flatten)]
    pub account: AccountOptions,
}

impl DeployArgs {
    pub async fn run(self, ui: &SozoUi) -> Result<()> {
        trace!(args = ?self);

        let DeployArgs {
            class_hash,
            salt,
            deployer_address,
            constructor_calldata,
            transaction,
            starknet,
            account,
        } = self;

        let constructor_felts = decode_calldata(&constructor_calldata)
            .context("Failed to parse constructor calldata")?;
        let expected_address =
            get_contract_address(salt, class_hash, &constructor_felts, deployer_address);

        let txn_config: TxnConfig = transaction.try_into()?;

        let account = get_account_from_env(account, &starknet).await?;

        ui.title(format!("Deploy contract (class hash {:#066x})", class_hash));
        ui.step("Deploying contract via UDC");
        let params_ui = ui.subsection();
        params_ui.verbose(format!("Class hash : {:#066x}", class_hash));
        params_ui.verbose(format!("Salt       : {salt:#066x}"));
        params_ui.verbose(format!("Deployer   : {deployer_address:#066x}"));
        params_ui.verbose(format!("Expect addr: {expected_address:#066x}"));
        if constructor_felts.is_empty() {
            params_ui.verbose("Constructor calldata: <empty>");
        } else {
            params_ui.verbose(format!("Constructor felts ({})", constructor_felts.len()));
            for (idx, value) in constructor_felts.iter().enumerate() {
                params_ui.verbose(params_ui.indent(1, format!("[{idx}] {value:#066x}")));
            }
        }

        let deployer = Deployer::new(account, txn_config);
        let (actual_address, tx_result) =
            deployer.deploy_via_udc(class_hash, salt, &constructor_felts, deployer_address).await?;

        match tx_result {
            TransactionResult::Noop => {
                let address =
                    if actual_address == Felt::ZERO { expected_address } else { actual_address };
                ui.result(format!("Contract already deployed.\n  Address   : {address:#066x}"));
                if address != expected_address {
                    ui.warn(format!(
                        "Computed address {expected_address:#066x} differs from on-chain \
                         {address:#066x}."
                    ));
                }
            }
            TransactionResult::Hash(hash) => {
                let deployed =
                    if actual_address == Felt::ZERO { expected_address } else { actual_address };
                ui.result(format!(
                    "Deployment submitted.\n  Tx hash   : {hash:#066x}\n  Address   : \
                     {deployed:#066x}"
                ));
                if deployed != expected_address {
                    ui.warn(format!(
                        "Computed address {expected_address:#066x} differs from on-chain \
                         {deployed:#066x}."
                    ));
                }
            }
            TransactionResult::HashReceipt(hash, receipt) => {
                let deployed =
                    if actual_address == Felt::ZERO { expected_address } else { actual_address };
                ui.result(format!(
                    "Contract deployed onchain.\n  Tx hash   : {hash:#066x}\n  Address   : \
                     {deployed:#066x}"
                ));
                if deployed != expected_address {
                    ui.warn(format!(
                        "Computed address {expected_address:#066x} differs from on-chain \
                         {deployed:#066x}."
                    ));
                }
                ui.debug(format!("Receipt: {:?}", receipt));
            }
        }

        Ok(())
    }
}
