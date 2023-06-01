use anyhow::Result;
use starknet::accounts::{Call, ConnectedAccount};
use starknet::core::types::FieldElement;
use starknet::core::utils::get_selector_from_name;
use starknet::providers::Provider;

use super::object::{
    ClassMigration, ContractMigration, Declarable, DeclareOutput, DeployOutput, Deployable,
    WorldContractMigration,
};
use crate::config::WorldConfig;
use crate::migration::object::MigrationError;

pub type MigrationResult<S, P> = Result<MigrationOutput, MigrationError<S, P>>;

#[derive(Debug)]
pub struct MigrationOutput {
    pub world: Option<DeployOutput>,
    pub executor: Option<DeployOutput>,
    pub systems: Vec<DeclareOutput>,
    pub components: Vec<DeclareOutput>,
}

#[derive(Debug)]
pub struct MigrationStrategy {
    pub world: Option<WorldContractMigration>,
    pub executor: Option<ContractMigration>,
    pub systems: Vec<ClassMigration>,
    pub components: Vec<ClassMigration>,
    pub world_config: WorldConfig,
}

impl MigrationStrategy {
    fn world_address(&self) -> Option<FieldElement> {
        match &self.world {
            Some(WorldContractMigration(c)) => c.contract_address,
            None => self.world_config.address,
        }
    }
}

impl MigrationStrategy {
    pub async fn execute<A>(
        &mut self,
        migrator: A,
    ) -> MigrationResult<A::SignError, <A::Provider as Provider>::Error>
    where
        A: ConnectedAccount + Sync,
    {
        if let Some(executor) = &mut self.executor {
            let res = executor.deploy(vec![], &migrator).await?;
        }

        if let Some(world) = &mut self.world {
            world
                .deploy(&migrator, self.executor.as_ref().unwrap().contract_address.unwrap())
                .await;
        }

        self.register_systems(&migrator).await?;
        self.register_components(&migrator).await?;

        Ok(())
    }

    async fn register_components<A>(&self, migrator: &A) -> Result<()>
    where
        A: ConnectedAccount + Sync,
    {
        for component in &self.components {
            component.declare(migrator).await;
        }

        let world_address = self.world_address().unwrap();

        let calls = self
            .components
            .iter()
            .map(|c| Call {
                to: world_address,
                selector: get_selector_from_name("register_component").unwrap(),
                calldata: vec![c.class.local],
            })
            .collect::<Vec<_>>();

        migrator
            .execute(calls)
            .send()
            .await
            .unwrap_or_else(|err| panic!("problem registering components: {err}"));

        Ok(())
    }

    async fn register_systems<A>(&self, migrator: &A) -> Result<()>
    where
        A: ConnectedAccount + Sync,
    {
        for system in &self.systems {
            system.declare(migrator).await;
        }

        let world_address = self.world_address().unwrap();

        let calls = self
            .systems
            .iter()
            .map(|s| Call {
                to: world_address,
                selector: get_selector_from_name("register_system").unwrap(),
                calldata: vec![s.class.local],
            })
            .collect::<Vec<_>>();

        migrator
            .execute(calls)
            .send()
            .await
            .unwrap_or_else(|err| panic!("problem registering systems: {err}"));

        Ok(())
    }
}
