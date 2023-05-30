pub mod world;

use std::fs::File;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use cairo_lang_starknet::casm_contract_class::CasmContractClass;
use cairo_lang_starknet::contract_class::ContractClass;
use starknet::accounts::{Account, Call, ConnectedAccount, SingleOwnerAccount};
use starknet::core::types::contract::{CompiledClass, SierraClass};
use starknet::core::types::{BlockId, BlockTag, FieldElement, FlattenedSierraClass};
use starknet::core::utils::{get_contract_address, get_selector_from_name};
use starknet::providers::jsonrpc::{HttpTransport, JsonRpcClient};
use starknet::providers::Provider;
use starknet::signers::LocalWallet;

use self::world::{Class, Contract};

// TODO: evaluate the contract address when building the migration plan
#[derive(Debug, Default)]
pub struct ContractMigration {
    pub deployed: bool,
    pub salt: FieldElement,
    pub contract: Contract,
    pub artifact_path: PathBuf,
    pub contract_address: Option<FieldElement>,
}

#[derive(Debug, Default)]
pub struct ClassMigration {
    pub declared: bool,
    pub class: Class,
    pub artifact_path: PathBuf,
}

// TODO: migration error
// should only be created by calling `World::prepare_for_migration`
pub struct Migration {
    world: ContractMigration,
    executor: ContractMigration,
    systems: Vec<ClassMigration>,
    components: Vec<ClassMigration>,
    migrator: SingleOwnerAccount<JsonRpcClient<HttpTransport>, LocalWallet>,
}

impl Migration {
    pub async fn execute(&mut self) -> Result<()> {
        if self.world.deployed {
            unimplemented!("migrate: branch -> if world is deployed")
        } else {
            self.migrate_full_world().await?;
        }

        Ok(())
    }

    async fn migrate_full_world(&mut self) -> Result<()> {
        if !self.executor.deployed {
            self.executor.deploy(vec![], &self.migrator).await;
        }

        self.world.deploy(vec![self.executor.contract_address.unwrap()], &self.migrator).await;

        self.register_components().await?;
        self.register_systems().await?;

        Ok(())
    }

    async fn register_components(&self) -> Result<()> {
        for component in &self.components {
            component.declare(&self.migrator).await;
        }

        let world_address = self
            .world
            .contract_address
            .unwrap_or_else(|| panic!("World contract address not found"));

        let calls = self
            .components
            .iter()
            .map(|c| Call {
                to: world_address,
                selector: get_selector_from_name("register_component").unwrap(),
                calldata: vec![c.class.local],
            })
            .collect::<Vec<_>>();

        self.migrator.execute(calls).send().await?;

        Ok(())
    }

    async fn register_systems(&self) -> Result<()> {
        for system in &self.systems {
            system.declare(&self.migrator).await;
        }

        let world_address = self
            .world
            .contract_address
            .unwrap_or_else(|| panic!("World contract address not found"));

        let calls = self
            .systems
            .iter()
            .map(|s| Call {
                to: world_address,
                selector: get_selector_from_name("register_system").unwrap(),
                calldata: vec![s.class.local],
            })
            .collect::<Vec<_>>();

        self.migrator.execute(calls).send().await?;

        Ok(())
    }
}

#[async_trait]
trait Declarable {
    async fn declare(
        &self,
        account: &SingleOwnerAccount<JsonRpcClient<HttpTransport>, LocalWallet>,
    );
}

// TODO: Remove `mut` once we can calculate the contract address before sending the tx
#[async_trait]
trait Deployable: Declarable {
    async fn deploy(
        &mut self,
        constructor_params: Vec<FieldElement>,
        account: &SingleOwnerAccount<JsonRpcClient<HttpTransport>, LocalWallet>,
    );
}

#[async_trait]
impl Declarable for ClassMigration {
    async fn declare(
        &self,
        account: &SingleOwnerAccount<JsonRpcClient<HttpTransport>, LocalWallet>,
    ) {
        declare(self.class.name.clone(), &self.artifact_path, account).await;
    }
}

#[async_trait]
impl Declarable for ContractMigration {
    async fn declare(
        &self,
        account: &SingleOwnerAccount<JsonRpcClient<HttpTransport>, LocalWallet>,
    ) {
        declare(self.contract.name.clone(), &self.artifact_path, account).await;
    }
}

async fn declare(
    name: String,
    artifact_path: &PathBuf,
    account: &SingleOwnerAccount<JsonRpcClient<HttpTransport>, LocalWallet>,
) {
    let (flattened_class, casm_class_hash) = prepare_contract_declaration_params(artifact_path)
        .unwrap_or_else(|err| {
            panic!("Preparing declaration for {name} class: {err}");
        });

    if account.provider().get_class(BlockId::Tag(BlockTag::Pending), casm_class_hash).await.is_ok()
    {
        println!("{name} class already declared");
        return;
    }

    let result = account.declare(Arc::new(flattened_class), casm_class_hash).send().await;

    match result {
        Ok(result) => {
            println!("Declared `{}` class at transaction: {:#x}", name, result.transaction_hash);
        }
        Err(error) => {
            if error.to_string().contains("already declared") {
                println!("{name} class already declared")
            } else {
                panic!("Problem declaring {name} class: {error}");
            }
        }
    }
}

#[async_trait]
impl Deployable for ContractMigration {
    async fn deploy(
        &mut self,
        constructor_calldata: Vec<FieldElement>,
        account: &SingleOwnerAccount<JsonRpcClient<HttpTransport>, LocalWallet>,
    ) {
        self.declare(account).await;

        let calldata = [
            vec![
                self.contract.local,                            // class hash
                self.salt,                                      // salt
                FieldElement::ZERO,                             // unique
                FieldElement::from(constructor_calldata.len()), // constructor calldata len
            ],
            constructor_calldata.clone(),
        ]
        .concat();

        let contract_address = get_contract_address(
            self.salt,
            self.contract.local,
            &constructor_calldata,
            FieldElement::ZERO,
        );

        self.contract_address = Some(contract_address);

        if account
            .provider()
            .get_class_hash_at(BlockId::Tag(BlockTag::Pending), contract_address)
            .await
            .is_ok()
        {
            self.deployed = true;
            println!("{} contract already deployed", self.contract.name);
            return;
        }

        println!("Deploying `{}` contract", self.contract.name);

        let res = account
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
            .unwrap_or_else(|e| panic!("problem deploying `{}` contract: {e}", self.contract.name));

        println!(
            "Deployed `{}` contract at transaction: {:#x}",
            self.contract.name, res.transaction_hash
        );
        println!("`{} `Contract address: {contract_address:#x}", self.contract.name);

        self.deployed = true;
    }
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
