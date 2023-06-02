use anyhow::Result;
use starknet::accounts::{Call, ConnectedAccount};
use starknet::core::types::{FieldElement, InvokeTransactionResult};
use starknet::providers::Provider;

use super::object::{
    ClassMigration, ContractMigration, Declarable, DeployOutput, Deployable, RegisterOutput,
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
            Some(executor) => executor.deploy(vec![], &migrator).await.map(|o| Some(o))?,
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

        let calls = self
            .components
            .iter()
            .map(|c| Call {
                to: world_address,
                // function selector: "register_component"
                selector: FieldElement::from_mont([
                    11981012454229264524,
                    8784065169116922201,
                    15056747385353365869,
                    456849768949735353,
                ]),
                calldata: vec![c.class.local],
            })
            .collect::<Vec<_>>();

        let InvokeTransactionResult { transaction_hash } = migrator.execute(calls).send().await?;

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

        let calls = self
            .systems
            .iter()
            .map(|s| Call {
                to: world_address,
                // function selector: "register_system"
                selector: FieldElement::from_mont([
                    6581716859078500959,
                    16871126355047595269,
                    14219012428168968926,
                    473332093618875024,
                ]),
                calldata: vec![s.class.local],
            })
            .collect::<Vec<_>>();

        let InvokeTransactionResult { transaction_hash } = migrator.execute(calls).send().await?;

        Ok(RegisterOutput { transaction_hash, declare_output })
    }
}
