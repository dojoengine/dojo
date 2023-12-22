use std::fs::File;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use cairo_lang_starknet::casm_contract_class::CasmContractClass;
use cairo_lang_starknet::contract_class::ContractClass;
use starknet::accounts::{Account, AccountError, Call, ConnectedAccount, SingleOwnerAccount};
use starknet::core::types::contract::{CompiledClass, SierraClass};
use starknet::core::types::{
    BlockId, BlockTag, DeclareTransactionResult, FieldElement, FlattenedSierraClass, FunctionCall,
    InvokeTransactionResult, StarknetError,
};
use starknet::core::utils::{
    get_contract_address, get_selector_from_name, CairoShortStringToFeltError,
};
use starknet::macros::{felt, selector};
use starknet::providers::{Provider, ProviderError};
use starknet::signers::Signer;
use thiserror::Error;

use crate::utils::{TransactionWaiter, TransactionWaitingError};

pub mod class;
pub mod contract;
pub mod strategy;
pub mod world;

pub type DeclareOutput = DeclareTransactionResult;

#[derive(Clone, Debug)]
pub struct DeployOutput {
    pub transaction_hash: FieldElement,
    pub contract_address: FieldElement,
    pub declare: Option<DeclareOutput>,
}

#[derive(Debug)]
pub struct RegisterOutput {
    pub transaction_hash: FieldElement,
    pub declare_output: Vec<DeclareOutput>,
}

#[derive(Debug, Error)]
pub enum MigrationError<S> {
    #[error("Compiling contract.")]
    CompilingContract,
    #[error("Class already declared.")]
    ClassAlreadyDeclared,
    #[error("Contract already deployed.")]
    ContractAlreadyDeployed(FieldElement),
    #[error(transparent)]
    Migrator(#[from] AccountError<S>),
    #[error(transparent)]
    CairoShortStringToFelt(#[from] CairoShortStringToFeltError),
    #[error(transparent)]
    Provider(#[from] ProviderError),
    #[error(transparent)]
    WaitingError(#[from] TransactionWaitingError),
}

/// Represents the type of migration that should be performed.
#[derive(Debug)]
pub enum MigrationType {
    /// When the remote class/contract already exists and has
    /// to be updated to match the local state.
    Update,
    /// When the class/contract does not exist on the remote state or
    /// when a new World is to be deployed.
    New,
}

pub trait StateDiff {
    /// Returns `true` if the local and remote states are equivalent.
    fn is_same(&self) -> bool;
}

/// The transaction configuration to use when sending a transaction.
#[derive(Debug, Copy, Clone, Default)]
pub struct TxConfig {
    /// The multiplier for how much the actual transaction max fee should be relative to the
    /// estimated fee. If `None` is provided, the multiplier is set to `1.1`.
    pub fee_estimate_multiplier: Option<f64>,
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait Declarable {
    async fn declare<P, S>(
        &self,
        account: &SingleOwnerAccount<P, S>,
        txn_config: TxConfig,
    ) -> Result<DeclareOutput, MigrationError<<SingleOwnerAccount<P, S> as Account>::SignError>>
    where
        P: Provider + Sync + Send,
        S: Signer + Sync + Send,
    {
        let (flattened_class, casm_class_hash) =
            prepare_contract_declaration_params(self.artifact_path()).unwrap();

        match account
            .provider()
            .get_class(BlockId::Tag(BlockTag::Pending), flattened_class.class_hash())
            .await
        {
            Err(ProviderError::StarknetError(StarknetError::ClassHashNotFound)) => {}
            Ok(_) => return Err(MigrationError::ClassAlreadyDeclared),
            Err(e) => return Err(MigrationError::Provider(e)),
        }

        let mut txn = account.declare(Arc::new(flattened_class), casm_class_hash);

        if let TxConfig { fee_estimate_multiplier: Some(multiplier) } = txn_config {
            txn = txn.fee_estimate_multiplier(multiplier);
        }

        let DeclareTransactionResult { transaction_hash, class_hash } =
            txn.send().await.map_err(MigrationError::Migrator)?;

        TransactionWaiter::new(transaction_hash, account.provider())
            .await
            .map_err(MigrationError::WaitingError)?;

        return Ok(DeclareOutput { transaction_hash, class_hash });
    }

    fn artifact_path(&self) -> &PathBuf;
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait Deployable: Declarable + Sync {
    async fn world_deploy<P, S>(
        &self,
        world_address: FieldElement,
        class_hash: FieldElement,
        account: &SingleOwnerAccount<P, S>,
        txn_config: TxConfig,
    ) -> Result<DeployOutput, MigrationError<<SingleOwnerAccount<P, S> as Account>::SignError>>
    where
        P: Provider + Sync + Send,
        S: Signer + Sync + Send,
    {
        let declare = match self.declare(account, txn_config).await {
            Ok(res) => Some(res),
            Err(MigrationError::ClassAlreadyDeclared) => None,
            Err(e) => return Err(e),
        };

        let base_class_hash = account
            .provider()
            .call(
                FunctionCall {
                    contract_address: world_address,
                    calldata: vec![],
                    entry_point_selector: get_selector_from_name("base").unwrap(),
                },
                BlockId::Tag(BlockTag::Pending),
            )
            .await
            .map_err(MigrationError::Provider)?;

        let contract_address =
            get_contract_address(self.salt(), base_class_hash[0], &[], world_address);

        let call = match account
            .provider()
            .get_class_hash_at(BlockId::Tag(BlockTag::Pending), contract_address)
            .await
        {
            Ok(current_class_hash) if current_class_hash != class_hash => Call {
                calldata: vec![contract_address, class_hash],
                selector: selector!("upgrade_contract"),
                to: world_address,
            },

            Err(ProviderError::StarknetError(StarknetError::ContractNotFound)) => Call {
                calldata: vec![self.salt(), class_hash],
                selector: selector!("deploy_contract"),
                to: world_address,
            },

            Ok(_) => {
                return Err(MigrationError::ContractAlreadyDeployed(contract_address));
            }

            Err(e) => return Err(MigrationError::Provider(e)),
        };

        let mut txn = account.execute(vec![call]);

        if let TxConfig { fee_estimate_multiplier: Some(multiplier) } = txn_config {
            txn = txn.fee_estimate_multiplier(multiplier);
        }

        let InvokeTransactionResult { transaction_hash } =
            txn.send().await.map_err(MigrationError::Migrator)?;

        TransactionWaiter::new(transaction_hash, account.provider()).await?;

        Ok(DeployOutput { transaction_hash, contract_address, declare })
    }

    async fn deploy<P, S>(
        &self,
        class_hash: FieldElement,
        constructor_calldata: Vec<FieldElement>,
        account: &SingleOwnerAccount<P, S>,
        txn_config: TxConfig,
    ) -> Result<DeployOutput, MigrationError<<SingleOwnerAccount<P, S> as Account>::SignError>>
    where
        P: Provider + Sync + Send,
        S: Signer + Sync + Send,
    {
        let declare = match self.declare(account, txn_config).await {
            Ok(res) => Some(res),
            Err(MigrationError::ClassAlreadyDeclared) => None,
            Err(e) => return Err(e),
        };

        let calldata = [
            vec![
                class_hash,                                     // class hash
                self.salt(),                                    // salt
                FieldElement::ZERO,                             // unique
                FieldElement::from(constructor_calldata.len()), // constructor calldata len
            ],
            constructor_calldata.clone(),
        ]
        .concat();

        let contract_address = get_contract_address(
            self.salt(),
            class_hash,
            &constructor_calldata,
            FieldElement::ZERO,
        );

        match account
            .provider()
            .get_class_hash_at(BlockId::Tag(BlockTag::Pending), contract_address)
            .await
        {
            Err(ProviderError::StarknetError(StarknetError::ContractNotFound)) => {}
            Ok(_) => return Err(MigrationError::ContractAlreadyDeployed(contract_address)),
            Err(e) => return Err(MigrationError::Provider(e)),
        }

        let mut txn = account.execute(vec![Call {
            calldata,
            // devnet UDC address
            selector: selector!("deployContract"),
            to: felt!("0x41a78e741e5af2fec34b695679bc6891742439f7afb8484ecd7766661ad02bf"),
        }]);

        if let TxConfig { fee_estimate_multiplier: Some(multiplier) } = txn_config {
            txn = txn.fee_estimate_multiplier(multiplier);
        }

        let InvokeTransactionResult { transaction_hash } =
            txn.send().await.map_err(MigrationError::Migrator)?;

        TransactionWaiter::new(transaction_hash, account.provider()).await?;

        Ok(DeployOutput { transaction_hash, contract_address, declare })
    }

    fn salt(&self) -> FieldElement;
}

fn prepare_contract_declaration_params(
    artifact_path: &PathBuf,
) -> Result<(FlattenedSierraClass, FieldElement)> {
    let flattened_class = read_class(artifact_path)?
        .flatten()
        .map_err(|e| anyhow!("error flattening the contract class: {e}"))?;
    let compiled_class_hash = get_compiled_class_hash(artifact_path).map_err(|e| {
        anyhow!("error computing compiled class hash: {} {e}", artifact_path.to_str().unwrap())
    })?;
    Ok((flattened_class, compiled_class_hash))
}

pub fn read_class(artifact_path: &PathBuf) -> Result<SierraClass> {
    let file = File::open(artifact_path)?;
    let contract_artifact: SierraClass = serde_json::from_reader(&file)?;
    Ok(contract_artifact)
}

fn get_compiled_class_hash(artifact_path: &PathBuf) -> Result<FieldElement> {
    let file = File::open(artifact_path)?;
    let casm_contract_class: ContractClass = serde_json::from_reader(file)?;
    let casm_contract = CasmContractClass::from_contract_class(casm_contract_class, true)?;
    let res = serde_json::to_string_pretty(&casm_contract)?;
    let compiled_class: CompiledClass = serde_json::from_str(&res)?;
    Ok(compiled_class.class_hash()?)
}
