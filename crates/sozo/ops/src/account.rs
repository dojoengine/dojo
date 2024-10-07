use core::panic;
use std::io::Write;
use std::path::PathBuf;

use anyhow::{Context, Result};
use colored::Colorize;
use colored_json::{ColorMode, Output};
use dojo_utils::{
    EthFeeSetting, FeeSetting, StrkFeeSetting, TokenFeeSetting, TransactionExtETH,
    TransactionExtSTRK, TransactionWaiter, TxnConfig,
};
use num_traits::{FromPrimitive, ToPrimitive};
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
use starknet_crypto::Felt;

use crate::utils;

/// The canonical hash of a contract class. This is the class hash value of a contract instance.
pub type ClassHash = Felt;

/// The class hash of DEFAULT_OZ_ACCOUNT_CONTRACT.
pub const DEFAULT_OZ_ACCOUNT_CONTRACT_CLASS_HASH: ClassHash =
    felt!("0x05400e90f7e0ae78bd02c77cd75527280470e2fe19c54970dd79dc37a9d3645c");

#[derive(Serialize, Deserialize, Debug)]
pub struct AccountConfig {
    pub version: u64,
    pub variant: AccountVariant,
    pub deployment: DeploymentStatus,
}

impl AccountConfig {
    pub fn deploy_account_address(&self) -> Result<Felt> {
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
                Felt::ZERO,
            )),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AccountVariant {
    OpenZeppelin(OzAccountConfig),
}

#[serde_as]
#[derive(Serialize, Deserialize, Debug)]
pub struct OzAccountConfig {
    pub version: u64,
    #[serde_as(as = "UfeHex")]
    pub public_key: Felt,
    #[serde(default = "true_as_default")]
    pub legacy: bool,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum DeploymentStatus {
    Undeployed(UndeployedStatus),
    Deployed(DeployedStatus),
}

impl DeploymentStatus {
    pub fn to_deployed(&mut self, address: Felt) {
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
#[derive(Serialize, Deserialize, Debug)]
pub struct UndeployedStatus {
    #[serde_as(as = "UfeHex")]
    pub class_hash: Felt,
    #[serde_as(as = "UfeHex")]
    pub salt: Felt,
}

#[serde_as]
#[derive(Serialize, Deserialize, Debug)]
pub struct DeployedStatus {
    #[serde_as(as = "UfeHex")]
    pub class_hash: Felt,
    #[serde_as(as = "UfeHex")]
    pub address: Felt,
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

#[allow(clippy::too_many_arguments)]
pub async fn deploy(
    provider: JsonRpcClient<HttpTransport>,
    signer: LocalWallet,
    txn_config: TxnConfig,
    nonce: Option<Felt>,
    poll_interval: u64,
    file: PathBuf,
    no_confirmation: bool,
) -> Result<()> {
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

    let target_deployment_address = account.deploy_account_address()?;
    let account_deployment_tx = match txn_config.fee_setting {
        FeeSetting::Eth(fee_setting) => {
            let account_deployment = factory.deploy_v1(undeployed_status.salt);

            // Sanity check. We don't really need to check again here actually
            if account_deployment.address() != target_deployment_address {
                anyhow::bail!("Unexpected account deployment address mismatch");
            }

            let account_deployment = match nonce {
                Some(nonce) => account_deployment.nonce(nonce),
                None => account_deployment,
            };

            let account_deployment_tx = match fee_setting {
                TokenFeeSetting::Send(fee) => {
                    match fee {
                        EthFeeSetting::Manual { max_fee_raw } => {
                            eprintln!(
                                "You've manually specified the account deployment fee to be {}. \
                                 Therefore, fund at least:\n    {}",
                                format!("{} ETH", utils::felt_to_bigdecimal(max_fee_raw, 18))
                                    .bright_yellow(),
                                format!("{} ETH", utils::felt_to_bigdecimal(max_fee_raw, 18))
                                    .bright_yellow(),
                            );
                        }
                        EthFeeSetting::Estimate { fee_estimate_multiplier } => {
                            let fee_estimate_multiplier = fee_estimate_multiplier.unwrap_or(1.1);
                            let estimated_fee = account_deployment
                                .estimate_fee()
                                .await
                                .map_err(|err| match err {
                                    AccountFactoryError::Provider(
                                        ProviderError::StarknetError(err),
                                    ) => map_starknet_error(err),
                                    err => anyhow::anyhow!("{}", err),
                                })?
                                .overall_fee;

                            let estimated_fee_with_buffer: Felt =
                                (((estimated_fee.to_u64().context("Invalid u64")? as f64)
                                    * fee_estimate_multiplier)
                                    as u64)
                                    .into();

                            eprintln!(
                                "The estimated account deployment fee is {} STRK. However, to \
                                 avoid failure, fund atleast:\n    {}",
                                format!("{} ETH", utils::felt_to_bigdecimal(estimated_fee, 18))
                                    .bright_yellow(),
                                format!(
                                    "{} ETH",
                                    utils::felt_to_bigdecimal(estimated_fee_with_buffer, 18)
                                )
                                .bright_yellow()
                            );
                        }
                    }

                    do_account_deploy_v1(
                        fee,
                        target_deployment_address,
                        no_confirmation,
                        account_deployment,
                    )
                    .await?
                }
                TokenFeeSetting::EstimateOnly => {
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

                    let decimal = utils::felt_to_bigdecimal(estimated_fee, 18);
                    println!("{} ETH", format!("{decimal}").bright_yellow());
                    return Ok(());
                }
            };

            account_deployment_tx
        }
        FeeSetting::Strk(fee_setting) => {
            let account_deployment = factory.deploy_v3(undeployed_status.salt);

            // Sanity check. We don't really need to check again here actually
            if account_deployment.address() != target_deployment_address {
                anyhow::bail!("Unexpected account deployment address mismatch");
            }

            let account_deployment = match nonce {
                Some(nonce) => account_deployment.nonce(nonce),
                None => account_deployment,
            };

            let account_deployment_tx = match fee_setting {
                TokenFeeSetting::Send(fee) => {
                    match fee {
                        StrkFeeSetting::Manual { gas, gas_price } => {
                            let fee_estimate = account_deployment.estimate_fee().await.map_err(
                                |err| match err {
                                    AccountFactoryError::Provider(
                                        ProviderError::StarknetError(err),
                                    ) => map_starknet_error(err),
                                    err => anyhow::anyhow!("{}", err),
                                },
                            )?;

                            let gas = if let Some(gas) = gas {
                                let gas = Felt::from_u64(gas).unwrap();
                                if fee_estimate.gas_consumed > gas {
                                    anyhow::bail!("gas consumed more than the limit");
                                } else {
                                    gas
                                }
                            } else {
                                fee_estimate.gas_consumed
                            };

                            let gas_price = if let Some(gas_price) = gas_price {
                                let gas_price = Felt::from_u128(gas_price).unwrap();
                                if fee_estimate.gas_price > gas_price {
                                    anyhow::bail!("gas price higher than limit");
                                } else {
                                    gas_price
                                }
                            } else {
                                fee_estimate.gas_price
                            };

                            let overall_fee = (gas * gas_price)
                                + (fee_estimate.data_gas_consumed * fee_estimate.data_gas_price);
                            eprintln!(
                                "You've manually specified the account deployment fee to be {}. \
                                 Therefore, fund at least:\n    {}",
                                format!("{} STRK", utils::felt_to_bigdecimal(overall_fee, 18))
                                    .bright_yellow(),
                                format!("{} STRK", utils::felt_to_bigdecimal(overall_fee, 18))
                                    .bright_yellow(),
                            );
                        }
                        StrkFeeSetting::Estimate { gas_estimate_multiplier } => {
                            let gas_estimate_multiplier = gas_estimate_multiplier.unwrap_or(1.1);
                            let estimated_fee = account_deployment
                                .estimate_fee()
                                .await
                                .map_err(|err| match err {
                                    AccountFactoryError::Provider(
                                        ProviderError::StarknetError(err),
                                    ) => map_starknet_error(err),
                                    err => anyhow::anyhow!("{}", err),
                                })?
                                .overall_fee;

                            let estimated_fee_with_buffer: Felt =
                                (((estimated_fee.to_u64().context("Invalid u64")? as f64)
                                    * gas_estimate_multiplier)
                                    as u64)
                                    .into();

                            eprintln!(
                                "The estimated account deployment fee is {} STRK. However, to \
                                 avoid failure, fund least:\n    {}",
                                format!("{} STRK", utils::felt_to_bigdecimal(estimated_fee, 18))
                                    .bright_yellow(),
                                format!(
                                    "{} STRK",
                                    utils::felt_to_bigdecimal(estimated_fee_with_buffer, 18)
                                )
                                .bright_yellow()
                            );
                        }
                    }
                    do_account_deploy_v3(
                        fee,
                        target_deployment_address,
                        no_confirmation,
                        account_deployment,
                    )
                    .await?
                }
                TokenFeeSetting::EstimateOnly => {
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

                    let decimal = utils::felt_to_bigdecimal(estimated_fee, 18);
                    println!("{} STRK", format!("{decimal}").bright_yellow());
                    return Ok(());
                }
            };

            account_deployment_tx
        }
    };

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
    let receipt = TransactionWaiter::new(account_deployment_tx, &provider)
        .with_tx_status(TransactionFinalityStatus::AcceptedOnL2)
        .with_interval(poll_interval)
        .await?;

    eprintln!(
        "Transaction {} confirmed",
        format!("{:#064x}", account_deployment_tx).bright_yellow()
    );

    if txn_config.receipt {
        println!("Receipt:\n{}", serde_json::to_string_pretty(&receipt)?);
    }

    account.deployment.to_deployed(target_deployment_address);

    write_account_to_file(file, account)?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn do_account_deploy_v1(
    fee_setting: EthFeeSetting,
    target_deployment_address: Felt,
    no_confirmation: bool,
    account_deployment: starknet::accounts::AccountDeploymentV1<
        '_,
        OpenZeppelinAccountFactory<LocalWallet, &JsonRpcClient<HttpTransport>>,
    >,
) -> Result<Felt, anyhow::Error> {
    eprintln!(
        "to the following address:\n    {}",
        format!("{:#064x}", target_deployment_address).bright_yellow()
    );

    if !no_confirmation {
        eprint!("Press [ENTER] once you've funded the address.");
        std::io::stdin().read_line(&mut String::new())?;
    }

    let account_deployment_tx =
        account_deployment.send_with_cfg(&fee_setting).await?.transaction_hash;

    return Ok(account_deployment_tx);
}

#[allow(clippy::too_many_arguments)]
async fn do_account_deploy_v3(
    fee_setting: StrkFeeSetting,
    target_deployment_address: Felt,
    no_confirmation: bool,
    account_deployment: starknet::accounts::AccountDeploymentV3<
        '_,
        OpenZeppelinAccountFactory<LocalWallet, &JsonRpcClient<HttpTransport>>,
    >,
) -> Result<Felt, anyhow::Error> {
    eprintln!(
        "to the following address:\n    {}",
        format!("{:#064x}", target_deployment_address).bright_yellow()
    );

    if !no_confirmation {
        eprint!("Press [ENTER] once you've funded the address.");
        std::io::stdin().read_line(&mut String::new())?;
    }

    let account_deployment_tx =
        account_deployment.send_with_cfg(&fee_setting).await?.transaction_hash;

    return Ok(account_deployment_tx);
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

#[allow(dead_code)]
async fn simulate_account_deploy(
    account_deployment: &starknet::accounts::AccountDeploymentV1<
        '_,
        OpenZeppelinAccountFactory<LocalWallet, &JsonRpcClient<HttpTransport>>,
    >,
) -> Result<(), anyhow::Error> {
    let simulation = account_deployment.simulate(false, false).await?;
    let simulation_json = serde_json::to_value(simulation)?;
    let simulation_json =
        colored_json::to_colored_json(&simulation_json, ColorMode::Auto(Output::StdOut))?;

    println!("{simulation_json}");
    Ok(())
}

pub async fn fetch(
    provider: JsonRpcClient<HttpTransport>,
    force: bool,
    output: PathBuf,
    address: Felt,
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
