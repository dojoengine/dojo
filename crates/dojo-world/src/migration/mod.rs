use std::fs::File;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use cairo_lang_starknet_classes::casm_contract_class::CasmContractClass;
use cairo_lang_starknet_classes::contract_class::ContractClass;
use starknet::accounts::{Account, AccountError, Call, ConnectedAccount, SingleOwnerAccount};
use starknet::core::types::contract::{CompiledClass, SierraClass};
use starknet::core::types::{
    BlockId, BlockTag, DeclareTransactionResult, FieldElement, FlattenedSierraClass,
    InvokeTransactionResult, MaybePendingTransactionReceipt, StarknetError, TransactionReceipt,
};
use starknet::core::utils::{get_contract_address, CairoShortStringToFeltError};
use starknet::macros::{felt, selector};
use starknet::providers::{Provider, ProviderError};
use starknet::signers::Signer;
use thiserror::Error;

use crate::utils::{TransactionExt, TransactionWaiter, TransactionWaitingError};

pub mod class;
pub mod contract;
pub mod strategy;
pub mod world;

pub type DeclareOutput = DeclareTransactionResult;

// TODO: this was taken from the current network limit
// https://docs.starknet.io/documentation/tools/limits_and_triggers/.
// This constant is also used into `katana-primitives`, which common crate
// should expose this value to have it configurable.
// Or should we just accept any size here?
pub const MAX_BYTECODE_SIZE: usize = 81_290;

#[derive(Clone, Debug)]
pub struct DeployOutput {
    pub transaction_hash: FieldElement,
    pub block_number: Option<u64>,
    pub contract_address: FieldElement,
    pub declare: Option<DeclareOutput>,
    // base class hash at time of deployment
    pub base_class_hash: FieldElement,
    pub was_upgraded: bool,
    pub name: Option<String>,
}

#[derive(Clone, Debug)]
pub struct UpgradeOutput {
    pub transaction_hash: FieldElement,
    pub block_number: Option<u64>,
    pub contract_address: FieldElement,
    pub declare: Option<DeclareOutput>,
}

#[derive(Debug)]
pub struct RegisterOutput {
    pub transaction_hash: FieldElement,
    pub declare_output: Vec<DeclareOutput>,
    pub registered_model_names: Vec<String>,
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
    #[error(transparent)]
    ArtifactError(#[from] anyhow::Error),
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
pub struct TxnConfig {
    /// The multiplier for how much the actual transaction max fee should be relative to the
    /// estimated fee. If `None` is provided, the multiplier is set to `1.1`.
    pub fee_estimate_multiplier: Option<f64>,
    pub wait: bool,
    pub receipt: bool,
}

impl TxnConfig {
    pub fn init_wait() -> Self {
        Self { wait: true, ..Default::default() }
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait Declarable {
    async fn declare<P, S>(
        &self,
        account: &SingleOwnerAccount<P, S>,
        txn_config: &TxnConfig,
    ) -> Result<DeclareOutput, MigrationError<<SingleOwnerAccount<P, S> as Account>::SignError>>
    where
        P: Provider + Sync + Send,
        S: Signer + Sync + Send,
    {
        let (flattened_class, casm_class_hash) =
            prepare_contract_declaration_params(self.artifact_path())?;

        match account
            .provider()
            .get_class(BlockId::Tag(BlockTag::Pending), flattened_class.class_hash())
            .await
        {
            Err(ProviderError::StarknetError(StarknetError::ClassHashNotFound)) => {}
            Ok(_) => return Err(MigrationError::ClassAlreadyDeclared),
            Err(e) => return Err(MigrationError::Provider(e)),
        }

        let DeclareTransactionResult { transaction_hash, class_hash } = account
            .declare(Arc::new(flattened_class), casm_class_hash)
            .send_with_cfg(txn_config)
            .await
            .map_err(MigrationError::Migrator)?;

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
    async fn deploy_dojo_contract<P, S>(
        &self,
        world_address: FieldElement,
        class_hash: FieldElement,
        base_class_hash: FieldElement,
        account: &SingleOwnerAccount<P, S>,
        txn_config: &TxnConfig,
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

        let contract_address =
            get_contract_address(self.salt(), base_class_hash, &[], world_address);

        let mut was_upgraded = false;

        let call = match account
            .provider()
            .get_class_hash_at(BlockId::Tag(BlockTag::Pending), contract_address)
            .await
        {
            Ok(current_class_hash) if current_class_hash != class_hash => {
                was_upgraded = true;

                Call {
                    calldata: vec![contract_address, class_hash],
                    selector: selector!("upgrade_contract"),
                    to: world_address,
                }
            }

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

        let InvokeTransactionResult { transaction_hash } = account
            .execute(vec![call])
            .send_with_cfg(txn_config)
            .await
            .map_err(MigrationError::Migrator)?;

        let receipt = TransactionWaiter::new(transaction_hash, account.provider()).await?;
        let block_number = get_block_number_from_receipt(receipt);

        Ok(DeployOutput {
            transaction_hash,
            block_number,
            contract_address,
            declare,
            base_class_hash,
            was_upgraded,
            name: None,
        })
    }

    async fn deploy<P, S>(
        &self,
        class_hash: FieldElement,
        constructor_calldata: Vec<FieldElement>,
        account: &SingleOwnerAccount<P, S>,
        txn_config: &TxnConfig,
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

        let txn = account.execute(vec![Call {
            calldata,
            // devnet UDC address
            selector: selector!("deployContract"),
            to: felt!("0x41a78e741e5af2fec34b695679bc6891742439f7afb8484ecd7766661ad02bf"),
        }]);

        let InvokeTransactionResult { transaction_hash } =
            txn.send_with_cfg(txn_config).await.map_err(MigrationError::Migrator)?;

        let receipt = TransactionWaiter::new(transaction_hash, account.provider()).await?;
        let block_number = get_block_number_from_receipt(receipt);

        Ok(DeployOutput {
            transaction_hash,
            block_number,
            contract_address,
            declare,
            base_class_hash: FieldElement::default(),
            was_upgraded: false,
            name: None,
        })
    }

    fn salt(&self) -> FieldElement;
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait Upgradable: Deployable + Declarable + Sync {
    async fn upgrade_world<P, S>(
        &self,
        class_hash: FieldElement,
        original_class_hash: FieldElement,
        original_base_class_hash: FieldElement,
        account: &SingleOwnerAccount<P, S>,
        txn_config: &TxnConfig,
    ) -> Result<UpgradeOutput, MigrationError<<SingleOwnerAccount<P, S> as Account>::SignError>>
    where
        P: Provider + Sync + Send,
        S: Signer + Sync + Send,
    {
        let declare = match self.declare(account, txn_config).await {
            Ok(res) => Some(res),
            Err(MigrationError::ClassAlreadyDeclared) => None,
            Err(e) => return Err(e),
        };

        let original_constructor_calldata = vec![original_base_class_hash];
        let contract_address = get_contract_address(
            self.salt(),
            original_class_hash,
            &original_constructor_calldata,
            FieldElement::ZERO,
        );

        match account
            .provider()
            .get_class_hash_at(BlockId::Tag(BlockTag::Pending), contract_address)
            .await
        {
            Ok(_) => {}
            Err(e) => return Err(MigrationError::Provider(e)),
        }

        let calldata = vec![class_hash];

        let InvokeTransactionResult { transaction_hash } = account
            .execute(vec![Call { calldata, selector: selector!("upgrade"), to: contract_address }])
            .send_with_cfg(txn_config)
            .await
            .map_err(MigrationError::Migrator)?;

        let receipt = TransactionWaiter::new(transaction_hash, account.provider()).await?;
        let block_number = get_block_number_from_receipt(receipt);

        Ok(UpgradeOutput { transaction_hash, block_number, contract_address, declare })
    }
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
    let casm_contract =
        CasmContractClass::from_contract_class(casm_contract_class, true, MAX_BYTECODE_SIZE)?;
    let res = serde_json::to_string_pretty(&casm_contract)?;
    let compiled_class: CompiledClass = serde_json::from_str(&res)?;
    Ok(compiled_class.class_hash()?)
}

fn get_block_number_from_receipt(receipt: MaybePendingTransactionReceipt) -> Option<u64> {
    match receipt {
        MaybePendingTransactionReceipt::Receipt(receipt) => match receipt {
            TransactionReceipt::Deploy(r) => Some(r.block_number),
            TransactionReceipt::Invoke(r) => Some(r.block_number),
            _ => None,
        },
        MaybePendingTransactionReceipt::PendingReceipt(_receipt) => None,
    }
}
