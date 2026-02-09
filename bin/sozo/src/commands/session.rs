use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use anyhow::{Result, anyhow};
use clap::{Args, Subcommand};
use dojo_world::contracts::ContractInfo;
use scarb_metadata::Metadata;
use scarb_metadata_ext::MetadataDojoExt;
use slot::account_sdk::provider::CartridgeJsonRpcProvider;
use sozo_ui::SozoUi;
use starknet::providers::Provider;
use tracing::trace;

use super::options::account::controller;
use super::options::starknet::StarknetOptions;
use super::options::world::WorldOptions;
use crate::utils;

#[derive(Debug, Args)]
pub struct SessionArgs {
    #[command(subcommand)]
    command: SessionCommand,
}

#[derive(Debug, Subcommand)]
pub enum SessionCommand {
    #[command(about = "Create or refresh a controller session from project contracts.")]
    Create {
        #[arg(long)]
        #[arg(help = "Load contracts from world diff (chain) instead of local manifest.")]
        diff: bool,

        #[command(flatten)]
        starknet: StarknetOptions,

        #[command(flatten)]
        world: WorldOptions,
    },

    #[command(about = "Show current controller session status for the selected network.")]
    Status {
        #[command(flatten)]
        starknet: StarknetOptions,
    },

    #[command(about = "Discard stored controller session(s).")]
    Discard {
        #[arg(long)]
        #[arg(help = "Discard all stored sessions for the authenticated controller account.")]
        all: bool,

        #[command(flatten)]
        starknet: StarknetOptions,
    },
}

impl SessionArgs {
    pub async fn run(self, scarb_metadata: &Metadata, ui: &SozoUi) -> Result<()> {
        trace!(args = ?self);

        match self.command {
            SessionCommand::Create { diff, starknet, world } => {
                create_session(diff, starknet, world, scarb_metadata, ui).await
            }
            SessionCommand::Status { starknet } => {
                status_session(starknet, scarb_metadata, ui).await
            }
            SessionCommand::Discard { all, starknet } => {
                discard_session(all, starknet, scarb_metadata, ui).await
            }
        }
    }
}

async fn create_session(
    diff: bool,
    starknet: StarknetOptions,
    world: WorldOptions,
    scarb_metadata: &Metadata,
    ui: &SozoUi,
) -> Result<()> {
    ui.title("Create controller session");

    let profile_config = scarb_metadata.load_dojo_profile_config()?;
    let rpc_url = starknet.url(profile_config.env.as_ref())?;
    let contracts = load_contracts(diff, starknet.clone(), world, scarb_metadata, ui).await?;

    ui.step("Authorize and register session");
    let rpc_provider = CartridgeJsonRpcProvider::new(rpc_url.clone());
    let chain_id = rpc_provider.chain_id().await?;
    let _ = controller::create_controller(rpc_url, rpc_provider, &contracts).await?;

    let session = slot::session::get(chain_id)?
        .ok_or_else(|| anyhow!("Session was not found in local storage after creation."))?;

    let session_path = session_file_path(&session.auth.username, chain_id);
    ui.result("Session is ready.");
    ui.print(format!("Controller address: {:#066x}", session.auth.address));
    ui.print(format!("Chain id          : {chain_id:#x}"));
    ui.print(format!("Policies          : {}", session.session.proved_policies.len()));
    ui.print(format!("Expires at (unix) : {}", session.session.inner.expires_at));
    ui.print(format!("Stored session    : {}", session_path.display()));
    ui.print(
        "Use `sozo execute ... --session` (or `--slot.controller`) to execute with this session.",
    );

    Ok(())
}

async fn status_session(
    starknet: StarknetOptions,
    scarb_metadata: &Metadata,
    ui: &SozoUi,
) -> Result<()> {
    ui.title("Controller session status");

    let credentials = match slot::credential::Credentials::load() {
        Ok(credentials) => credentials,
        Err(
            slot::Error::Unauthorized
            | slot::Error::MalformedCredentials
            | slot::Error::InvalidOAuth,
        ) => {
            ui.warn("No controller credentials found. Run `sozo session create` first.");
            return Ok(());
        }
        Err(err) => return Err(err.into()),
    };

    let profile_config = scarb_metadata.load_dojo_profile_config()?;
    let rpc_url = starknet.url(profile_config.env.as_ref())?;
    let chain_id = CartridgeJsonRpcProvider::new(rpc_url).chain_id().await?;

    ui.print(format!("Account id        : {}", credentials.account.id));
    ui.print(format!("Username          : {}", credentials.account.username));
    ui.print(format!("Chain id          : {chain_id:#x}"));

    if let Some(controller) = credentials.account.controllers.first() {
        ui.print(format!("Controller address: {:#066x}", controller.address));
    } else {
        ui.warn("No controller is associated with the authenticated account.");
    }

    let session_path = session_file_path(&credentials.account.id, chain_id);
    let session = slot::session::get(chain_id)?;

    if let Some(session) = session {
        ui.result("Session: active");
        ui.print(format!("Policies          : {}", session.session.proved_policies.len()));
        ui.print(format!("Expires at (unix) : {}", session.session.inner.expires_at));
        ui.print(format!("Stored session    : {}", session_path.display()));
    } else {
        ui.warn("Session: not found for this network.");
        ui.print(format!("Expected path     : {}", session_path.display()));
    }

    Ok(())
}

async fn discard_session(
    all: bool,
    starknet: StarknetOptions,
    scarb_metadata: &Metadata,
    ui: &SozoUi,
) -> Result<()> {
    ui.title("Discard controller session");

    let credentials = match slot::credential::Credentials::load() {
        Ok(credentials) => credentials,
        Err(
            slot::Error::Unauthorized
            | slot::Error::MalformedCredentials
            | slot::Error::InvalidOAuth,
        ) => {
            ui.warn("No controller credentials found.");
            return Ok(());
        }
        Err(err) => return Err(err.into()),
    };

    let mut removed = 0usize;
    if all {
        let user_dir = slot::utils::config_dir().join(&credentials.account.id);
        if user_dir.exists() {
            for entry in fs::read_dir(&user_dir)? {
                let path = entry?.path();
                let is_session = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.ends_with("-session.json"));

                if is_session {
                    fs::remove_file(&path)?;
                    removed += 1;
                }
            }
        }

        ui.result(format!("Discarded {removed} session(s)."));
        return Ok(());
    }

    let profile_config = scarb_metadata.load_dojo_profile_config()?;
    let rpc_url = starknet.url(profile_config.env.as_ref())?;
    let chain_id = CartridgeJsonRpcProvider::new(rpc_url).chain_id().await?;

    let session_path = session_file_path(&credentials.account.id, chain_id);
    if session_path.exists() {
        fs::remove_file(&session_path)?;
        ui.result("Session discarded.");
        ui.print(format!("Removed: {}", session_path.display()));
    } else {
        ui.warn("No stored session found for this network.");
        ui.print(format!("Expected path: {}", session_path.display()));
    }

    Ok(())
}

async fn load_contracts(
    diff: bool,
    starknet: StarknetOptions,
    world: WorldOptions,
    scarb_metadata: &Metadata,
    ui: &SozoUi,
) -> Result<HashMap<String, ContractInfo>> {
    if diff {
        let (world_diff, _, _) =
            utils::get_world_diff_and_provider(starknet, world, scarb_metadata, ui).await?;
        return Ok((&world_diff).into());
    }

    let manifest = scarb_metadata.read_dojo_manifest_profile()?.ok_or_else(|| {
        anyhow!(
            "Project manifest not found. Run `sozo migrate` first or pass `--diff` to derive \
             contracts from chain."
        )
    })?;

    Ok((&manifest).into())
}

fn session_file_path(username: &str, chain_id: starknet::core::types::Felt) -> PathBuf {
    slot::utils::config_dir().join(username).join(format!("{chain_id:#x}-session.json"))
}

#[cfg(test)]
mod tests {
    use starknet::macros::felt;

    use super::session_file_path;

    #[test]
    fn session_file_path_contains_expected_suffix() {
        let path = session_file_path("my-user", felt!("0x534e5f5345504f4c4941"));
        let file = path.file_name().and_then(|name| name.to_str()).unwrap();
        assert!(file.ends_with("-session.json"));
    }
}
