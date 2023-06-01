use anyhow::{bail, Result};
use starknet::accounts::{Account, Call, SingleOwnerAccount};
use starknet::core::types::FieldElement;
use starknet::core::utils::get_selector_from_name;
use starknet::providers::Provider;
use starknet::signers::Signer;

use super::object::{
    ClassMigration, ContractMigration, Declarable, Deployable, WorldContractMigration,
};
use crate::config::WorldConfig;
use crate::migration::object::MigrationError;

pub struct MigrationStrategy {
    pub world: Option<WorldContractMigration>,
    pub executor: Option<ContractMigration>,
    pub systems: Vec<ClassMigration>,
    pub components: Vec<ClassMigration>,
    pub world_config: WorldConfig,
}

impl MigrationStrategy {
    fn world_address(&self) -> Result<FieldElement> {
        if self.world.is_none() && self.world_config.address.is_none() {
            bail!(MigrationError::WorldAddressNotFound)
        }

        Ok(match &self.world {
            // Right now we optimistically assume that if the World contract is to be migrated,
            // then the world address should exists because it would be deployed
            // first before this function is used.
            Some(WorldContractMigration(c)) if c.contract_address.is_some() => {
                c.contract_address.unwrap()
            }
            _ => self.world_config.address.unwrap(),
        })
    }
}

impl MigrationStrategy {
    pub async fn execute<P, S>(&mut self, migrator: SingleOwnerAccount<P, S>) -> Result<()>
    where
        P: Provider + Send + Sync,
        S: Signer + Send + Sync,
    {
        if let Some(executor) = &mut self.executor {
            executor.deploy(vec![], &migrator).await;
        }

        if let Some(world) = &mut self.world {
            world
                .deploy(
                    "my world",
                    self.executor.as_ref().unwrap().contract_address.unwrap(),
                    &migrator,
                )
                .await;
        }

        self.register_systems(&migrator).await?;
        self.register_components(&migrator).await?;

        Ok(())
    }

    async fn register_components<P, S>(&self, migrator: &SingleOwnerAccount<P, S>) -> Result<()>
    where
        P: Provider + Send + Sync,
        S: Signer + Send + Sync,
    {
        for component in &self.components {
            component.declare(migrator).await;
        }

        let world_address = self.world_address()?;

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

    async fn register_systems<P, S>(&self, migrator: &SingleOwnerAccount<P, S>) -> Result<()>
    where
        P: Provider + Send + Sync,
        S: Signer + Send + Sync,
    {
        for system in &self.systems {
            system.declare(migrator).await;
        }

        let world_address = self.world_address()?;

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
