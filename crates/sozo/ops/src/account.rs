use core::panic;
use std::io::Write;
use std::path::PathBuf;

use anyhow::Result;
use colored::Colorize;
use colored_json::{ColorMode, Output};
use dojo_world::utils::TransactionWaiter;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use starknet::accounts::{AccountFactory, AccountFactoryError, OpenZeppelinAccountFactory};
use starknet::core::serde::unsigned_field_element::UfeHex;
use starknet::core::types::{
    BlockId, BlockTag, FunctionCall, StarknetError, TransactionFinalityStatus,
};
use starknet::core::utils::get_contract_address;
use starknet::macros::{felt, selector};
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::{JsonRpcClient, Provider, ProviderError};
use starknet::signers::{LocalWallet, Signer, SigningKey};
use starknet_crypto::FieldElement;

/// The canonical hash of a contract class. This is the class hash value of a contract instance.
pub type ClassHash = FieldElement;

/// The class hash of DEFAULT_OZ_ACCOUNT_CONTRACT.
/// Corresponds to 0x05400e90f7e0ae78bd02c77cd75527280470e2fe19c54970dd79dc37a9d3645c
pub const DEFAULT_OZ_ACCOUNT_CONTRACT_CLASS_HASH: ClassHash = FieldElement::from_mont([
    8460675502047588988,
    17729791148444280953,
    7171298771336181387,
    292243705759714441,
]);

#[derive(Serialize, Deserialize)]
pub struct AccountConfig {
    pub version: u64,
    pub variant: AccountVariant,
    pub deployment: DeploymentStatus,
}

impl AccountConfig {
    pub fn deploy_account_address(&self) -> Result<FieldElement> {
        let undeployed_status = match &self.deployment {
            DeploymentStatus::Undeployed(value) => value,
            DeploymentStatus::Deployed(_) => {
                anyhow::bail!("account already deployed");
            }
        };

        match &self.variant {
            AccountVariant::OpenZeppelin(oz) => Ok(get_contract_address(
                undeployed_status.salt,
                undeployed_status.class_hash,
                &[oz.public_key],
                FieldElement::ZERO,
            )),
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AccountVariant {
    OpenZeppelin(OzAccountConfig),
}

#[serde_as]
#[derive(Serialize, Deserialize)]
pub struct OzAccountConfig {
    pub version: u64,
    #[serde_as(as = "UfeHex")]
    pub public_key: FieldElement,
    #[serde(default = "true_as_default")]
    pub legacy: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum DeploymentStatus {
    Undeployed(UndeployedStatus),
    Deployed(DeployedStatus),
}

impl DeploymentStatus {
    pub fn to_deployed(&mut self, address: FieldElement) {
        match self {
            DeploymentStatus::Undeployed(status) => {
                *self = DeploymentStatus::Deployed(DeployedStatus {
                    class_hash: status.class_hash,
                    address,
                });
            }
            DeploymentStatus::Deployed(_) => {
                panic!("Already deployed!")
            }
        }
    }
}

#[serde_as]
#[derive(Serialize, Deserialize)]
pub struct UndeployedStatus {
    #[serde_as(as = "UfeHex")]
    pub class_hash: FieldElement,
    #[serde_as(as = "UfeHex")]
    pub salt: FieldElement,
}

#[serde_as]
#[derive(Serialize, Deserialize)]
pub struct DeployedStatus {
    #[serde_as(as = "UfeHex")]
    pub class_hash: FieldElement,
    #[serde_as(as = "UfeHex")]
    pub address: FieldElement,
}

enum MaxFeeType {
    Manual { max_fee: FieldElement },
    Estimated { estimate: FieldElement, estimate_with_buffer: FieldElement },
}

impl MaxFeeType {
    pub fn max_fee(&self) -> FieldElement {
        match self {
            Self::Manual { max_fee } => *max_fee,
            Self::Estimated { estimate_with_buffer, .. } => *estimate_with_buffer,
        }
    }
}

#[derive(Debug)]
pub enum FeeSetting {
    Manual(FieldElement),
    EstimateOnly,
    None,
}

impl FeeSetting {
    pub fn is_estimate_only(&self) -> bool {
        matches!(self, FeeSetting::EstimateOnly)
    }
}

pub async fn new(signer: LocalWallet, force: bool, file: PathBuf) -> Result<()> {
    if file.exists() && !force {
        anyhow::bail!("account config file already exists");
    }

    let salt = SigningKey::from_random().secret_scalar();

    let account_config = AccountConfig {
        version: 1,
        variant: AccountVariant::OpenZeppelin(OzAccountConfig {
            version: 1,
            public_key: signer.get_public_key().await?.scalar(),
            legacy: false,
        }),
        deployment: DeploymentStatus::Undeployed(UndeployedStatus {
            class_hash: DEFAULT_OZ_ACCOUNT_CONTRACT_CLASS_HASH,
            salt,
        }),
    };

    let deployed_address = account_config.deploy_account_address()?;

    let file_path = file;
    let mut file = std::fs::File::create(&file_path)?;
    serde_json::to_writer_pretty(&mut file, &account_config)?;
    file.write_all(b"\n")?;

    eprintln!("Created new account config file: {}", std::fs::canonicalize(&file_path)?.display());
    eprintln!();
    eprintln!(
        "Once deployed, this account will be available at:\n    {}",
        format!("{:#064x}", deployed_address).bright_yellow()
    );
    eprintln!();
    eprintln!(
        "Deploy this account by running:\n    {}",
        format!("sozo account deploy {}", file_path.display()).bright_yellow()
    );

    Ok(())
}

pub async fn deploy(
    provider: JsonRpcClient<HttpTransport>,
    signer: LocalWallet,
    fee_setting: FeeSetting,
    simulate: bool,
    nonce: Option<FieldElement>,
    poll_interval: u64,
    file: PathBuf,
    no_confirmation: bool,
) -> Result<()> {
    if simulate && fee_setting.is_estimate_only() {
        anyhow::bail!("--simulate cannot be used with --estimate-only");
    }

    if !file.exists() {
        anyhow::bail!("account config file not found");
    }

    let mut account: AccountConfig = serde_json::from_reader(&mut std::fs::File::open(&file)?)?;

    let undeployed_status = match &account.deployment {
        DeploymentStatus::Undeployed(inner) => inner,
        DeploymentStatus::Deployed(_) => {
            anyhow::bail!("account already deployed");
        }
    };

    let chain_id = provider.chain_id().await?;

    let factory = match &account.variant {
        AccountVariant::OpenZeppelin(oz_config) => {
            // Makes sure we're using the right key
            let signer_public_key = signer.get_public_key().await?.scalar();
            if signer_public_key != oz_config.public_key {
                anyhow::bail!(
                    "public key mismatch. Expected: {:#064x}; actual: {:#064x}.",
                    oz_config.public_key,
                    signer_public_key
                );
            }

            let mut factory = OpenZeppelinAccountFactory::new(
                undeployed_status.class_hash,
                chain_id,
                signer,
                &provider,
            )
            .await?;
            factory.set_block_id(BlockId::Tag(BlockTag::Pending));

            factory
        }
    };

    let account_deployment = factory.deploy(undeployed_status.salt);

    let target_deployment_address = account.deploy_account_address()?;

    // Sanity check. We don't really need to check again here actually
    if account_deployment.address() != target_deployment_address {
        panic!("Unexpected account deployment address mismatch");
    }

    let max_fee = match fee_setting {
        FeeSetting::Manual(fee) => MaxFeeType::Manual { max_fee: fee },
        FeeSetting::EstimateOnly | FeeSetting::None => {
            let estimated_fee = account_deployment
                .estimate_fee()
                .await
                .map_err(|err| match err {
                    AccountFactoryError::Provider(ProviderError::StarknetError(err)) => {
                        map_starknet_error(err)
                    }
                    err => anyhow::anyhow!("{}", err),
                })?
                .overall_fee;

            let estimated_fee_with_buffer = (estimated_fee * felt!("3")).floor_div(felt!("2"));

            if fee_setting.is_estimate_only() {
                println!("{} ETH", format!("{}", estimated_fee.to_big_decimal(18)).bright_yellow(),);
                return Ok(());
            }

            MaxFeeType::Estimated {
                estimate: estimated_fee,
                estimate_with_buffer: estimated_fee_with_buffer,
            }
        }
    };

    let account_deployment = match nonce {
        Some(nonce) => account_deployment.nonce(nonce),
        None => account_deployment,
    };
    let account_deployment = account_deployment.max_fee(max_fee.max_fee());

    if simulate {
        simulate_account_deploy(&account_deployment).await?;
        return Ok(());
    } else {
        do_account_deploy(
            max_fee,
            target_deployment_address,
            no_confirmation,
            account_deployment,
            &provider,
            poll_interval,
            &mut account,
        )
        .await?;

        write_account_to_file(file, account)?;

        Ok(())
    }
}

async fn do_account_deploy(
    max_fee: MaxFeeType,
    target_deployment_address: FieldElement,
    no_confirmation: bool,
    account_deployment: starknet::accounts::AccountDeployment<
        '_,
        OpenZeppelinAccountFactory<LocalWallet, &JsonRpcClient<HttpTransport>>,
    >,
    provider: &JsonRpcClient<HttpTransport>,
    poll_interval: u64,
    account: &mut AccountConfig,
) -> Result<(), anyhow::Error> {
    match max_fee {
        MaxFeeType::Manual { max_fee } => {
            eprintln!(
                "You've manually specified the account deployment fee to be {}. Therefore, fund \
                 at least:\n    {}",
                format!("{} ETH", max_fee.to_big_decimal(18)).bright_yellow(),
                format!("{} ETH", max_fee.to_big_decimal(18)).bright_yellow(),
            );
        }
        MaxFeeType::Estimated { estimate, estimate_with_buffer } => {
            eprintln!(
                "The estimated account deployment fee is {}. However, to avoid failure, fund at \
                 least:\n    {}",
                format!("{} ETH", estimate.to_big_decimal(18)).bright_yellow(),
                format!("{} ETH", estimate_with_buffer.to_big_decimal(18)).bright_yellow()
            );
        }
    }
    eprintln!(
        "to the following address:\n    {}",
        format!("{:#064x}", target_deployment_address).bright_yellow()
    );
    if !no_confirmation {
        eprint!("Press [ENTER] once you've funded the address.");
        std::io::stdin().read_line(&mut String::new())?;
    }
    let account_deployment_tx = account_deployment.send().await?.transaction_hash;
    eprintln!(
        "Account deployment transaction: {}",
        format!("{:#064x}", account_deployment_tx).bright_yellow()
    );
    eprintln!(
        "Waiting for transaction {} to confirm. If this process is interrupted, you will need to \
         run `{}` to update the account file.",
        format!("{:#064x}", account_deployment_tx).bright_yellow(),
        "sozo account fetch".bright_yellow(),
    );
    TransactionWaiter::new(account_deployment_tx, &provider)
        .with_tx_status(TransactionFinalityStatus::AcceptedOnL2)
        .with_interval(poll_interval)
        .await?;
    eprintln!(
        "Transaction {} confirmed",
        format!("{:#064x}", account_deployment_tx).bright_yellow()
    );

    account.deployment.to_deployed(target_deployment_address);

    Ok(())
}

fn write_account_to_file(file: PathBuf, account: AccountConfig) -> Result<(), anyhow::Error> {
    let mut temp_file_name = file
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("unable to determine file name"))?
        .to_owned();

    // Never write directly to the original file to avoid data loss
    temp_file_name.push(".tmp");

    let mut temp_path = file.clone();
    temp_path.set_file_name(temp_file_name);

    let mut temp_file = std::fs::File::create(&temp_path)?;
    serde_json::to_writer_pretty(&mut temp_file, &account)?;
    // temp_file.write_all(b"\n")?;

    std::fs::rename(temp_path, file)?;
    Ok(())
}

async fn simulate_account_deploy(
    account_deployment: &starknet::accounts::AccountDeployment<
        '_,
        OpenZeppelinAccountFactory<LocalWallet, &JsonRpcClient<HttpTransport>>,
    >,
) -> Result<(), anyhow::Error> {
    let simulation = account_deployment.simulate(false, false).await?;
    let simulation_json = serde_json::to_value(simulation)?;
    let simulation_json =
        colored_json::to_colored_json(&simulation_json, ColorMode::Auto(Output::StdOut))?;

    println!("{simulation_json}");
    return Ok(());
}

pub async fn fetch(
    provider: JsonRpcClient<HttpTransport>,
    force: bool,
    output: PathBuf,
    address: FieldElement,
) -> Result<()> {
    if output.exists() && !force {
        anyhow::bail!("account config file already exists");
    }

    let class_hash = provider.get_class_hash_at(BlockId::Tag(BlockTag::Pending), address).await?;

    let public_key = provider
        .call(
            FunctionCall {
                contract_address: address,
                entry_point_selector: selector!("get_public_key"),
                calldata: vec![],
            },
            BlockId::Tag(BlockTag::Pending),
        )
        .await?[0];

    let variant =
        AccountVariant::OpenZeppelin(OzAccountConfig { version: 1, public_key, legacy: false });

    let account = AccountConfig {
        version: 1,
        variant,
        deployment: DeploymentStatus::Deployed(DeployedStatus { class_hash, address }),
    };

    let mut file = std::fs::File::create(&output)?;
    serde_json::to_writer_pretty(&mut file, &account)?;
    file.write_all(b"\n")?;

    eprintln!("Downloaded new account config file: {}", std::fs::canonicalize(&output)?.display());

    Ok(())
}

fn true_as_default() -> bool {
    true
}

fn map_starknet_error(err: StarknetError) -> anyhow::Error {
    match err {
        StarknetError::ContractError(err) => {
            anyhow::anyhow!("ContractError: {}", err.revert_error.trim())
        }
        StarknetError::TransactionExecutionError(err) => {
            anyhow::anyhow!(
                "TransactionExecutionError (tx index {}): {}",
                err.transaction_index,
                err.execution_error.trim()
            )
        }
        StarknetError::ValidationFailure(err) => {
            anyhow::anyhow!("ValidationFailure: {}", err.trim())
        }
        StarknetError::UnexpectedError(err) => {
            anyhow::anyhow!("UnexpectedError: {}", err.trim())
        }
        err => anyhow::anyhow!("{}", err),
    }
}
