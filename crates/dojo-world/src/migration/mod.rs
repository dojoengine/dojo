pub mod object;
pub mod state;

use std::fs::File;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use cairo_lang_starknet::casm_contract_class::CasmContractClass;
use cairo_lang_starknet::contract_class::ContractClass;
use starknet::accounts::{Account, Call, ConnectedAccount, SingleOwnerAccount};
use starknet::core::types::contract::{CompiledClass, FlattenedSierraClass, SierraClass};
use starknet::core::types::FieldElement;
use starknet::core::utils::{get_contract_address, get_selector_from_name};
use starknet::providers::jsonrpc::models::{BlockId, BlockTag};
use starknet::providers::jsonrpc::{HttpTransport, JsonRpcClient};
use starknet::signers::LocalWallet;

use self::object::{ClassMigration, ContractMigration, WorldContractMigration};
use self::state::{Class, Contract};

// TODO: migration error
// should only be created by calling `World::prepare_for_migration`
pub struct Migration {
    world: WorldContractMigration,
    executor: ContractMigration,

    // system and component can be declared and registered in parallel
    systems: Vec<ClassMigration>,
    components: Vec<ClassMigration>,

    // use generic `Provider`
    migrator: SingleOwnerAccount<JsonRpcClient<HttpTransport>, LocalWallet>,
}

// MIGRATION UI
// top level load bar : shows progress for the overall migration (how many contracts left to be deployed etc)
// individual loader: each contract types will have their own loader to display their migration progress

// MIGRATION CONFIRMATION LOGIC
// problem: how do we make sure that all contracts are successfully declared/deployed/registered ??
// need to keep track of all sent transaction hashes

impl Migration {
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
            .contract
            .address
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
            .contract
            .address
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
