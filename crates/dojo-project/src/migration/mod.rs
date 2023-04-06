pub mod world;

use std::{fs, path::PathBuf, sync::Arc};

use anyhow::Result;
use async_trait::async_trait;

use starknet::{
    accounts::{Account, Call, SingleOwnerAccount},
    core::{
        types::{contract::SierraClass, FieldElement},
        utils::cairo_short_string_to_felt,
    },
    providers::SequencerGatewayProvider,
    signers::LocalWallet,
};

use self::world::{Class, Contract};

// TODO: evaluate the contract address when building the migration plan
#[derive(Debug, Default)]
pub struct ContractMigration {
    pub deployed: bool,
    // pub salt: FieldElement,
    pub contract: Contract,
    pub artifact_path: PathBuf,
}

#[derive(Debug, Default)]
pub struct ClassMigration {
    pub declared: bool,
    pub class: Class,
    pub artifact_path: PathBuf,
}

// TODO: refactor type for Contract/Class migration
// TODO: include migrator account
// TODO: migration error
// TODO: migration config
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
        let indexer = {
            if !self.indexer.declared {
                self.indexer.declare(account).await;
            }
            self.indexer.class.local
        };

        let store = {
            if !self.store.declared {
                self.store.declare(account).await;
            }
            self.store.class.local
        };

        let executor = {
            if !self.executor.deployed {
                self.executor.deploy(vec![], account).await;
            }
            self.executor.contract.address.unwrap()
        };

        self.world.declare(account).await;
        self.world.deploy(vec![executor, store, indexer], account).await;

        self.register_components(account).await?;
        self.register_systems(account).await?;

        Ok(())
    }

    async fn register_components(
        &self,
        account: &SingleOwnerAccount<SequencerGatewayProvider, LocalWallet>,
    ) -> Result<()> {
        for component in &self.components {
            component.declare(&account).await;
        }

        let world_address = self
            .world
            .contract
            .address
            .unwrap_or_else(|| panic!("World contract address not found"));

        let calls = self
            .components
            .iter()
            .map(|c| {
                let class_hash = c.class.local;
                Call {
                    to: world_address,
                    selector: cairo_short_string_to_felt("register_component").unwrap(),
                    calldata: vec![class_hash],
                }
            })
            .collect::<Vec<_>>();

        // register components
        let _res = account.execute(calls).send().await?;

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
            .map(|s| {
                let class_hash = s.class.local;
                Call {
                    to: world_address,
                    selector: cairo_short_string_to_felt("register_system").unwrap(),
                    calldata: vec![class_hash],
                }
            })
            .collect::<Vec<_>>();

        let _res = account.execute(calls).send().await?;

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
                panic!("Problem deploying {} artifact: {}", self.class.name, error);
            });

        println!("class for `{}` deployed at tx `{}`", self.class.name, result.transaction_hash);

        //  can probably remove this part but just to be sure

        assert!(
            Some(self.class.local) == result.class_hash,
            "local and remote class hash should be equal"
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
                panic!("Problem deploying {} artifact: {}", self.contract.name, error);
            });

        println!(
            "Contract for `{}` deployed at tx `{}`",
            self.contract.name, result.transaction_hash
        );

        //  can probably remove this part but just to be sure

        assert!(
            Some(self.contract.local) == result.class_hash,
            "local and remote class hash should be equal"
        );
    }
}

#[async_trait]
impl Deployable for ContractMigration {
    async fn deploy(
        &mut self,
        constructor_params: Vec<FieldElement>,
        account: &SingleOwnerAccount<SequencerGatewayProvider, LocalWallet>,
    ) {
        self.declare(&account).await;

        let class_hash = self.contract.local;
        let calldata = [
            vec![
                class_hash,
                FieldElement::ZERO,
                FieldElement::ZERO,
                FieldElement::from(constructor_params.len()),
            ],
            constructor_params,
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
                selector: cairo_short_string_to_felt("deployContract").unwrap(),
            }])
            .send()
            .await
            .unwrap_or_else(|e| {
                panic!("problem deploying contract for `{}`: {e}", self.contract.name)
            });

        self.deployed = true;
        self.contract.address = res.address;

        println!(
            "Contract `{}` deployed at transaction hash `{}`",
            self.contract.name, res.transaction_hash
        );
    }
}

// TODO: create `utils` module
// fn compute_class_hash_of_contract_class(class: ContractClass) -> Result<FieldElement> {
//     let casm_contract = CasmContractClass::from_contract_class(class, true)?;
//     let class_json = serde_json::to_string_pretty(&casm_contract)?;
//     let compiled_class: CompiledClass = serde_json::from_str(&class_json)?;
//     compiled_class.class_hash().map_err(|e| e.into())
// }
