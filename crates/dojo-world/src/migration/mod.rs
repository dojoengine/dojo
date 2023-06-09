use std::fs::File;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use cairo_lang_starknet::casm_contract_class::CasmContractClass;
use cairo_lang_starknet::contract_class::ContractClass;
use starknet::accounts::{AccountError, Call, ConnectedAccount};
use starknet::core::types::contract::{CompiledClass, SierraClass};
use starknet::core::types::{
    BlockId, BlockTag, DeclareTransactionResult, FieldElement, FlattenedSierraClass,
    InvokeTransactionResult,
};
use starknet::core::utils::{
    get_contract_address, get_selector_from_name, CairoShortStringToFeltError,
};
use starknet::providers::Provider;
use thiserror::Error;

pub mod class;
pub mod contract;
pub mod world;

pub type DeclareOutput = DeclareTransactionResult;

#[derive(Debug)]
pub struct DeployOutput {
    pub transaction_hash: FieldElement,
    pub contract_address: FieldElement,
    pub declare_res: DeclareOutput,
}

#[derive(Debug)]
pub struct RegisterOutput {
    pub transaction_hash: FieldElement,
    pub declare_output: Vec<DeclareOutput>,
}

#[derive(Debug, Error)]
pub enum MigrationError<S, P> {
    #[error("Class already declared.")]
    ClassAlreadyDeclared,
    #[error("Contract already deployed.")]
    ContractAlreadyDeployed,
    #[error(transparent)]
    Migrator(#[from] AccountError<S, P>),
    #[error(transparent)]
    CairoShortStringToFelt(#[from] CairoShortStringToFeltError),
}

#[async_trait]
pub trait Declarable {
    async fn declare<A>(
        &self,
        account: &A,
    ) -> Result<DeclareOutput, MigrationError<A::SignError, <A::Provider as Provider>::Error>>
    where
        A: ConnectedAccount + Sync,
    {
        let (flattened_class, casm_class_hash) =
            prepare_contract_declaration_params(self.artifact_path()).unwrap();

        if account
            .provider()
            .get_class(BlockId::Tag(BlockTag::Pending), casm_class_hash)
            .await
            .is_ok()
        {
            return Err(MigrationError::ClassAlreadyDeclared);
        }

        account
            .declare(Arc::new(flattened_class), casm_class_hash)
            .send()
            .await
            .map_err(MigrationError::Migrator)
    }

    fn artifact_path(&self) -> &PathBuf;
}

// TODO: Remove `mut` once we can calculate the contract address before sending the tx
#[async_trait]
pub trait Deployable: Declarable + Sync {
    async fn deploy<A>(
        &mut self,
        constructor_calldata: Vec<FieldElement>,
        account: &A,
    ) -> Result<DeployOutput, MigrationError<A::SignError, <A::Provider as Provider>::Error>>
    where
        A: ConnectedAccount + Sync,
    {
        let declare_res = self.declare(account).await?;

        let calldata = [
            vec![
                declare_res.class_hash,                         // class hash
                FieldElement::ZERO,                             // salt
                FieldElement::ZERO,                             // unique
                FieldElement::from(constructor_calldata.len()), // constructor calldata len
            ],
            constructor_calldata.clone(),
        ]
        .concat();

        let contract_address = get_contract_address(
            FieldElement::ZERO,
            declare_res.class_hash,
            &constructor_calldata,
            FieldElement::ZERO,
        );

        self.set_contract_address(contract_address);

        if account
            .provider()
            .get_class_hash_at(BlockId::Tag(BlockTag::Pending), contract_address)
            .await
            .is_ok()
        {
            return Err(MigrationError::ContractAlreadyDeployed);
        }

        let InvokeTransactionResult { transaction_hash } = account
            .execute(vec![Call {
                calldata,
                // devnet UDC address
                to: FieldElement::from_hex_be(
                    "0x41a78e741e5af2fec34b695679bc6891742439f7afb8484ecd7766661ad02bf",
                )
                .unwrap(),
                selector: get_selector_from_name("deployContract").unwrap(),
            }])
            .send()
            .await
            .map_err(MigrationError::Migrator)?;

        Ok(DeployOutput { transaction_hash, contract_address, declare_res })
    }

    // TEMP: Remove once we can calculate the contract address before sending the tx
    fn set_contract_address(&mut self, contract_address: FieldElement);
}

fn prepare_contract_declaration_params(
    artifact_path: &PathBuf,
) -> Result<(FlattenedSierraClass, FieldElement)> {
    let flattened_class = get_flattened_class(artifact_path)
        .map_err(|e| anyhow!("error flattening the contract class: {e}"))?;
    let compiled_class_hash = get_compiled_class_hash(artifact_path)
        .map_err(|e| anyhow!("error computing compiled class hash: {e}"))?;
    Ok((flattened_class, compiled_class_hash))
}

fn get_flattened_class(artifact_path: &PathBuf) -> Result<FlattenedSierraClass> {
    let file = File::open(artifact_path)?;
    let contract_artifact: SierraClass = serde_json::from_reader(&file)?;
    Ok(contract_artifact.flatten()?)
}

fn get_compiled_class_hash(artifact_path: &PathBuf) -> Result<FieldElement> {
    let file = File::open(artifact_path)?;
    let casm_contract_class: ContractClass = serde_json::from_reader(file)?;
    let casm_contract = CasmContractClass::from_contract_class(casm_contract_class, true)
        .with_context(|| "Compilation failed.")?;
    let res = serde_json::to_string_pretty(&casm_contract)?;
    let compiled_class: CompiledClass = serde_json::from_str(&res)?;
    Ok(compiled_class.class_hash()?)
}
