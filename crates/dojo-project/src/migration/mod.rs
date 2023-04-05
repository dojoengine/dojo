pub mod world;

use anyhow::Result;
use async_trait::async_trait;

use starknet::{
    accounts::{Account, SingleOwnerAccount},
    core::types::FieldElement,
    providers::SequencerGatewayProvider,
    signers::LocalWallet,
};

use self::world::{Class, Contract};

#[async_trait]
trait Declarable {
    async fn declare(&self, account: SingleOwnerAccount<SequencerGatewayProvider, LocalWallet>);
}

#[async_trait]
trait Deployable: Declarable {
    async fn deploy(&self, account: SingleOwnerAccount<SequencerGatewayProvider, LocalWallet>);
}

#[derive(Debug, Default)]
pub struct ContractMigration {
    deployed: bool,
    salt: FieldElement,
    contract: Contract,
}

#[async_trait]
impl Declarable for ContractMigration {
    async fn declare(&self, account: SingleOwnerAccount<SequencerGatewayProvider, LocalWallet>) {
        // let contract =
        // account.declare(contract_class)
    }
}

#[derive(Debug, Default)]
pub struct ClassMigration {
    declared: bool,
    class: Class,
}

// TODO: migration config
#[derive(Debug, Default)]
pub struct Migration {
    // rpc: Deployments,
    url: String, // sequencer url for testing purposes atm
    world: ContractMigration,
    executor: ContractMigration,
    store: ClassMigration,
    indexer: ClassMigration,
    systems: Vec<ClassMigration>,
    components: Vec<ClassMigration>,
}

// should only be created by calling `World::prepare_for_migration`
impl Migration {
    pub async fn migrate(&self) -> Result<()> {
        // if self.world.deployed {}

        unimplemented!("world migration")
    }
}
