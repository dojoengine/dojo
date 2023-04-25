pub mod contract;
pub mod world;

use starknet::accounts::SingleOwnerAccount;
use starknet::providers::SequencerGatewayProvider;
use starknet::signers::LocalWallet;

use self::contract::{
    ClassMigration, ContractMigration, Declarable, Deployable, WorldContractMigration,
};

#[derive(Debug, thiserror::Error)]
pub enum MigrationError {
    #[error("Failed to deploy contract")]
    DeploymentError(#[source] anyhow::Error),
    #[error("Failed to declare contract")]
    DeclarationError(#[source] anyhow::Error),
    #[error("Failed to register executor to world")]
    ExecutorRegistrationError,
    #[error("Failed to register systems to world")]
    SystemsRegistrationError,
    #[error("Failed to register components to world")]
    ComponentsRegistrationError,
}

// should only be created by calling `World::prepare_for_migration`
pub struct Migration {
    world: WorldContractMigration,
    executor: ContractMigration,
    systems: Vec<ClassMigration>,
    components: Vec<ClassMigration>,
    migrator: SingleOwnerAccount<SequencerGatewayProvider, LocalWallet>,
}

impl Migration {
    pub async fn execute(&mut self) -> Result<(), MigrationError> {
        let mut executor_should_set = false;

        if !self.executor.deployed {
            self.executor
                .deploy(vec![], &self.migrator)
                .await
                .map_err(MigrationError::DeploymentError)?;

            executor_should_set = true;
        }

        if self.world.0.deployed {
            if executor_should_set {
                self.world
                    .set_executor(self.executor.contract_address.unwrap(), &self.migrator)
                    .await
                    .map_err(|_| MigrationError::ExecutorRegistrationError)?;
            }
        } else {
            self.world
                .deploy(self.executor.contract_address.unwrap(), &self.migrator)
                .await
                .map_err(MigrationError::DeploymentError)?;
        }

        self.register_components().await?;
        self.register_systems().await?;

        Ok(())
    }

    async fn register_components(&self) -> Result<(), MigrationError> {
        for component in &self.components {
            component.declare(&self.migrator).await.map_err(MigrationError::DeclarationError)?;
        }

        self.world
            .register_component(&self.components, &self.migrator)
            .await
            .map_err(|_| MigrationError::ComponentsRegistrationError)?;

        Ok(())
    }

    async fn register_systems(&self) -> Result<(), MigrationError> {
        for system in &self.systems {
            system.declare(&self.migrator).await.map_err(MigrationError::DeclarationError)?;
        }

        self.world
            .register_system(&self.systems, &self.migrator)
            .await
            .map_err(|_| MigrationError::SystemsRegistrationError)?;

        Ok(())
    }
}
