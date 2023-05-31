use anyhow::Result;
use starknet::accounts::{Account, Call, SingleOwnerAccount};
use starknet::core::utils::get_selector_from_name;
use starknet::providers::Provider;
use starknet::signers::Signer;
use thiserror::Error;

use super::object::{ClassMigration, ContractMigration, WorldContractMigration};

#[derive(Debug, Error)]
pub enum MigrationError {}

// TODO: migration error
// should only be created by calling `World::prepare_for_migration`
pub struct MigrationStrategy {
    world: Option<WorldContractMigration>,
    executor: Option<ContractMigration>,
    systems: Vec<ClassMigration>,
    components: Vec<ClassMigration>,
}

impl<P, S> MigrationStrategy<P, S>
where
    P: Provider + Send,
    S: Signer + Send,
{
    pub async fn execute(&mut self) -> Result<()> {
        if self.world.deployed {
            unimplemented!("migrate: branch -> if world is deployed")
        } else {
            self.migrate_full_world().await?;
        }

        Ok(())
    }

    async fn migrate_full_world(&mut self) -> Result<()> {
        if !self.executor.deployed {
            self.executor.deploy(vec![], &self.migrator).await;
        }

        self.world.deploy(vec![self.executor.contract_address.unwrap()], &self.migrator).await;

        self.register_components().await?;
        self.register_systems().await?;

        Ok(())
    }

    async fn register_components(&self) -> Result<()> {
        for component in &self.components {
            component.declare(&self.migrator).await;
        }

        let world_address = self
            .world
            .contract_address
            .unwrap_or_else(|| panic!("World contract address not found"));

        let calls = self
            .components
            .iter()
            .map(|c| Call {
                to: world_address,
                selector: get_selector_from_name("register_component").unwrap(),
                calldata: vec![c.class.local],
            })
            .collect::<Vec<_>>();

        self.migrator.execute(calls).send().await?;

        Ok(())
    }

    async fn register_systems(&self) -> Result<()> {
        for system in &self.systems {
            system.declare(&self.migrator).await;
        }

        let world_address = self
            .world
            .contract_address
            .unwrap_or_else(|| panic!("World contract address not found"));

        let calls = self
            .systems
            .iter()
            .map(|s| Call {
                to: world_address,
                selector: get_selector_from_name("register_system").unwrap(),
                calldata: vec![s.class.local],
            })
            .collect::<Vec<_>>();

        self.migrator.execute(calls).send().await?;

        Ok(())
    }
}
