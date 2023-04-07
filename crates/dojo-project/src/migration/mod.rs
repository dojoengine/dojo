pub mod world;

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use starknet::accounts::{Account, Call, SingleOwnerAccount};
use starknet::core::types::contract::SierraClass;
use starknet::core::types::FieldElement;
use starknet::core::utils::{get_contract_address, get_selector_from_name};
use starknet::providers::SequencerGatewayProvider;
use starknet::signers::LocalWallet;

use self::world::{Class, Contract};

// TODO: evaluate the contract address when building the migration plan
#[derive(Debug, Default)]
pub struct ContractMigration {
    pub deployed: bool,
    pub salt: FieldElement,
    pub contract: Contract,
    pub artifact_path: PathBuf,
    // not to be confused with `compiled_class_hash` fields in `contract`: `local` and `remote`
    pub class_hash: FieldElement,
    pub contract_address: Option<FieldElement>,
}

#[derive(Debug, Default)]
pub struct ClassMigration {
    pub declared: bool,
    pub class: Class,
    pub artifact_path: PathBuf,
    // not to be confused with `compiled_class_hash` fields in `class`: `local` and `remote`
    pub class_hash: FieldElement,
}

// TODO: migration error
// should only be created by calling `World::prepare_for_migration`
#[derive(Default)]
pub struct Migration {
    world: ContractMigration,
    executor: ContractMigration,
    store: ClassMigration,
    indexer: ClassMigration,
    systems: Vec<ClassMigration>,
    components: Vec<ClassMigration>,
}

impl Migration {
    pub async fn execute(
        &mut self,
        migrator: SingleOwnerAccount<SequencerGatewayProvider, LocalWallet>,
    ) -> Result<()> {
        if self.world.deployed {
            unimplemented!("migrate: branch -> if world is deployed")
        } else {
            self.migrate_full_world(&migrator).await?;
        }

        Ok(())
    }

    async fn migrate_full_world(
        &mut self,
        account: &SingleOwnerAccount<SequencerGatewayProvider, LocalWallet>,
    ) -> Result<()> {
        if !self.indexer.declared {
            self.indexer.declare(account).await;
        }

        if !self.store.declared {
            self.store.declare(account).await;
        }

        if !self.executor.deployed {
            self.executor.deploy(vec![], account).await;
        }

        self.world
            .deploy(
                vec![
                    self.executor.contract_address.unwrap(),
                    self.store.class_hash,
                    self.indexer.class_hash,
                ],
                account,
            )
            .await;

        self.register_components(account).await?;
        self.register_systems(account).await?;

        Ok(())
    }

    async fn register_components(
        &self,
        account: &SingleOwnerAccount<SequencerGatewayProvider, LocalWallet>,
    ) -> Result<()> {
        for component in &self.components {
            component.declare(account).await;
        }

        let world_address = self
            .world
            .contract
            .address
            .unwrap_or_else(|| panic!("World contract address not found"));

        let calls = self
            .components
            .iter()
            .map(|c| Call {
                to: world_address,
                selector: get_selector_from_name("register_component").unwrap(),
                calldata: vec![c.class_hash],
            })
            .collect::<Vec<_>>();

        account.execute(calls).send().await?;

        Ok(())
    }

    async fn register_systems(
        &self,
        account: &SingleOwnerAccount<SequencerGatewayProvider, LocalWallet>,
    ) -> Result<()> {
        for system in &self.systems {
            system.declare(account).await;
        }

        let world_address = self
            .world
            .contract
            .address
            .unwrap_or_else(|| panic!("World contract address not found"));

        let calls = self
            .systems
            .iter()
            .map(|s| Call {
                to: world_address,
                selector: get_selector_from_name("register_system").unwrap(),
                calldata: vec![s.class_hash],
            })
            .collect::<Vec<_>>();

        account.execute(calls).send().await?;

        Ok(())
    }
}

#[async_trait]
trait Declarable {
    async fn declare(&self, account: &SingleOwnerAccount<SequencerGatewayProvider, LocalWallet>);
}

// can remove `mut` once we can calculate the contract addres before sending the tx
#[async_trait]
trait Deployable: Declarable {
    async fn deploy(
        &mut self,
        constructor_params: Vec<FieldElement>,
        account: &SingleOwnerAccount<SequencerGatewayProvider, LocalWallet>,
    );
}

#[async_trait]
impl Declarable for ClassMigration {
    async fn declare(&self, account: &SingleOwnerAccount<SequencerGatewayProvider, LocalWallet>) {
        let contract_artifact =
            serde_json::from_reader::<_, SierraClass>(fs::File::open(&self.artifact_path).unwrap())
                .unwrap();
        let flattened_class = contract_artifact.flatten().unwrap();

        let result = account
            .declare(Arc::new(flattened_class), self.class.local)
            .send()
            .await
            .unwrap_or_else(|error| {
                panic!("Problem declaring {} contract: {error}", self.class.name);
            });

        println!(
            "Declared `{}` contract at transaction: {:#x}",
            self.class.name, result.transaction_hash
        );
    }
}

#[async_trait]
impl Declarable for ContractMigration {
    async fn declare(&self, account: &SingleOwnerAccount<SequencerGatewayProvider, LocalWallet>) {
        let contract_artifact =
            serde_json::from_reader::<_, SierraClass>(fs::File::open(&self.artifact_path).unwrap())
                .unwrap();
        let flattened_class = contract_artifact.flatten().unwrap();

        let result = account
            .declare(Arc::new(flattened_class), self.contract.local)
            .send()
            .await
            .unwrap_or_else(|error| {
                panic!("problem declaring {} contract: {error}", self.contract.name);
            });

        println!(
            "Declared `{}` contract at transaction: {:#x}",
            self.contract.name, result.transaction_hash
        );
    }
}

#[async_trait]
impl Deployable for ContractMigration {
    async fn deploy(
        &mut self,
        constructor_calldata: Vec<FieldElement>,
        account: &SingleOwnerAccount<SequencerGatewayProvider, LocalWallet>,
    ) {
        self.declare(account).await;

        let calldata = [
            vec![
                self.class_hash,                                // class hash
                self.salt,                                      // salt
                FieldElement::ZERO,                             // unique
                FieldElement::from(constructor_calldata.len()), // constructor calldata len
            ],
            constructor_calldata.clone(),
        ]
        .concat();

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

        let contract_address = get_contract_address(
            self.salt,
            self.class_hash,
            &constructor_calldata,
            FieldElement::ZERO,
        );

        self.contract_address = Some(contract_address);

        println!(
            "Deployed `{}` contract at transaction: {:#x}",
            self.contract.name, res.transaction_hash
        );
        println!("`{} `Contract address: {contract_address:#x}", self.contract.name);

        self.deployed = true;
        self.contract.address = Some(contract_address);
    }
}
