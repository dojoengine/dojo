pub mod world;

use std::path::PathBuf;

use anyhow::Result;
use async_trait::async_trait;

use starknet::{
    accounts::{Account, SingleOwnerAccount},
    core::types::FieldElement,
    providers::SequencerGatewayProvider,
    signers::LocalWallet,
};

use self::world::{Class, Contract};

#[derive(Debug, Default)]
pub struct ContractMigration {
    pub deployed: bool,
    pub salt: FieldElement,
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

// should only be created by calling `World::prepare_for_migration`
impl Migration {
    // sequencer url for testing purposes
    pub async fn migrate(&self, url: String) -> Result<()> {
        // if self.world.deployed {}

        unimplemented!("world migration")
    }
}

#[async_trait]
trait Declarable {
    async fn declare(&mut self, account: SingleOwnerAccount<SequencerGatewayProvider, LocalWallet>);
}

#[async_trait]
trait Deployable: Declarable {
    async fn deploy(
        &mut self,
        constructor_params: &[FieldElement],
        account: SingleOwnerAccount<SequencerGatewayProvider, LocalWallet>,
    );
}

#[async_trait]
impl Declarable for ContractMigration {
    async fn declare(
        &mut self,
        account: SingleOwnerAccount<SequencerGatewayProvider, LocalWallet>,
    ) {
        let name = self.contract.name.as_str();

        if matches!("World", "Executor") {

            // get the contract artifact from `release/target` folder
        } else {
        }
    }
}
