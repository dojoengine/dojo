use anyhow::Result;
use starknet::accounts::ConnectedAccount;
use starknet::core::types::{FieldElement, InvokeTransactionResult};
use starknet::providers::Provider;

use super::object::{
    ClassMigration, ContractMigration, Declarable, DeployOutput, Deployable, RegisterOutput,
    WorldContract,
};
use crate::config::WorldConfig;
use crate::migration::object::MigrationError;

pub type MigrationResult<S, P> = Result<MigrationOutput, MigrationError<S, P>>;

#[derive(Debug)]
pub struct MigrationOutput {
    pub world: Option<DeployOutput>,
    pub executor: Option<DeployOutput>,
    pub systems: RegisterOutput,
    pub components: RegisterOutput,
}

#[derive(Debug)]
pub struct MigrationStrategy {
    pub world: Option<ContractMigration>,
    pub executor: Option<ContractMigration>,
    pub systems: Vec<ClassMigration>,
    pub components: Vec<ClassMigration>,
    pub world_config: WorldConfig,
}

impl MigrationStrategy {
    fn world_address(&self) -> Option<FieldElement> {
        match &self.world {
            Some(c) => c.contract_address,
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
        let executor_output = match &mut self.executor {
            Some(executor) => {
                let res = executor.deploy(vec![], &migrator).await?;

                if self.world.is_none() {
                    let addr = self.world_address().ok_or(MigrationError::WorldAddressNotFound)?;
                    WorldContract::new(addr, &migrator).set_executor(res.contract_address).await?;
                }

                Some(res)
            }
            None => None,
        };

        let world_output = match &mut self.world {
            Some(world) => world
                .deploy(vec![self.executor.as_ref().unwrap().contract_address.unwrap()], &migrator)
                .await
                .map(|o| Some(o))?,
            None => None,
        };

        let components_output = self.register_systems(&migrator).await?;
        let systems_output = self.register_components(&migrator).await?;

        Ok(MigrationOutput {
            world: world_output,
            executor: executor_output,
            systems: systems_output,
            components: components_output,
        })
    }

    async fn register_components<A>(
        &self,
        migrator: &A,
    ) -> Result<RegisterOutput, MigrationError<A::SignError, <A::Provider as Provider>::Error>>
    where
        A: ConnectedAccount + Sync,
    {
        let mut declare_output = vec![];
        for component in &self.components {
            declare_output.push(component.declare(migrator).await?);
        }

        let world_address = self.world_address().ok_or(MigrationError::WorldAddressNotFound)?;

        let InvokeTransactionResult { transaction_hash } =
            WorldContract::new(world_address, migrator)
                .register_components(
                    &declare_output.iter().map(|o| o.class_hash).collect::<Vec<_>>(),
                )
                .await?;

        Ok(RegisterOutput { transaction_hash, declare_output })
    }

    async fn register_systems<A>(
        &self,
        migrator: &A,
    ) -> Result<RegisterOutput, MigrationError<A::SignError, <A::Provider as Provider>::Error>>
    where
        A: ConnectedAccount + Sync,
    {
        let mut declare_output = vec![];
        for system in &self.systems {
            declare_output.push(system.declare(migrator).await?);
        }

        let world_address = self.world_address().ok_or(MigrationError::WorldAddressNotFound)?;

        let InvokeTransactionResult { transaction_hash } =
            WorldContract::new(world_address, migrator)
                .register_components(
                    &declare_output.iter().map(|o| o.class_hash).collect::<Vec<_>>(),
                )
                .await?;

        Ok(RegisterOutput { transaction_hash, declare_output })
    }
}
