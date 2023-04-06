pub mod world;

use std::{fs, path::PathBuf, rc::Rc, sync::Arc};

use anyhow::{Context, Result};
use async_trait::async_trait;

use cairo_lang_starknet::{casm_contract_class::CasmContractClass, contract_class::ContractClass};
use starknet::{
    accounts::{Account, Call, SingleOwnerAccount},
    core::{
        chain_id,
        types::{contract::SierraClass, FieldElement},
        utils::cairo_short_string_to_felt,
    },
    providers::SequencerGatewayProvider,
    signers::{LocalWallet, SigningKey},
};
use url::Url;

use self::world::{Class, Contract};

// TODO: calculate the contract address before sending the tx
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

// TODO: migration config
// should only be created by calling `World::prepare_for_migration`
#[derive(Debug, Default)]
pub struct Migration {
    // rpc: Deployments,
    world: ContractMigration,
    executor: ContractMigration,
    store: ClassMigration,
    indexer: ClassMigration,
    systems: Vec<ClassMigration>,
    components: Vec<ClassMigration>,
}

impl Migration {
    // we use sequencer here because devnet still doesnt support cairo1 rpc
    pub async fn execute(&mut self, provider: SequencerGatewayProvider) -> Result<()> {
        let signer = LocalWallet::from(SigningKey::from_secret_scalar(
            FieldElement::from_hex_be("0x5d4fb5e2c807cd78ac51675e06be7099").unwrap(),
        ));
        let address = FieldElement::from_hex_be(
            "0x5f6fd2a43f4bce1bdfb2d0e9212d910227d9f67cf1425f2a9ceae231572c643",
        )
        .unwrap();
        let account = SingleOwnerAccount::new(provider, signer, address, chain_id::TESTNET);

        if self.world.deployed {
            unimplemented!("migrate: branch -> if world is deployed")
        } else {
            self.migrate_full_world(&account).await?;
        }

        Ok(())
    }

    async fn migrate_full_world(
        &mut self,
        account: &SingleOwnerAccount<SequencerGatewayProvider, LocalWallet>,
    ) -> Result<()> {
        // Can safely unwrap the values here because `World::prepare_for_migration` should ensure
        // this has value if `declared` == true, and `Declarable::declare` and `Deployable::deploy`
        // should also set the value accordingly.

        let indexer = {
            if !self.indexer.declared {
                self.indexer.declare(&account).await;
            }
            self.indexer.class.remote.unwrap()
        };

        let store = {
            if !self.store.declared {
                self.store.declare(&account).await;
            }
            self.store.class.remote.unwrap()
        };

        let executor = {
            if !self.executor.deployed {
                self.executor.deploy(vec![], &account).await;
            }
            self.executor.contract.address.unwrap()
        };

        self.world.declare(&account).await;
        self.world.deploy(vec![executor, store, indexer], &account).await;

        self.register_components(&account).await?;
        self.register_systems(&account).await?;

        Ok(())
    }

    async fn register_components(
        &self,
        account: &SingleOwnerAccount<SequencerGatewayProvider, LocalWallet>,
    ) -> Result<()> {
        // declare every component
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
        // declare every system
        for system in &self.systems {
            system.declare(&account).await;
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
                    selector: cairo_short_string_to_felt("register_component").unwrap(),
                    calldata: vec![class_hash],
                }
            })
            .collect::<Vec<_>>();

        // register systems
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
