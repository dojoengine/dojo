use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::str::FromStr;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use dojo_world::contracts::contract_info::ContractInfo;
use serde::{Deserialize, Serialize};
use slot::account_sdk::account::session::account::SessionAccount;
use slot::account_sdk::account::session::merkle::MerkleTree;
use slot::account_sdk::account::session::policy::{CallPolicy, MerkleLeaf, Policy, ProvedPolicy};
use slot::account_sdk::hash::MessageHashRev1;
use slot::account_sdk::provider::CartridgeJsonRpcProvider;
use slot::session::{FullSessionInfo, PolicyMethod};
use starknet::core::types::{BlockId, BlockTag, Felt, FunctionCall};
use starknet::core::utils::get_selector_from_name;
use starknet::macros::felt;
use starknet::providers::Provider;
use tracing::trace;
use url::Url;

#[allow(missing_debug_implementations)]
pub type ControllerAccount = SessionAccount;

const CONTROLLER_OAUTH_TIMEOUT_SECS: u64 = 300;
const CONTROLLER_OAUTH_CALLBACK_PATH: &str = "/callback";
const CONTROLLER_LOGIN_PATH: &str = "/slot";
const CONTROLLER_SESSION_REGISTRATION_TIMEOUT_SECS: u64 = 60;
const CONTROLLER_SESSION_REGISTRATION_POLL_MS: u64 = 1_500;
const CONTROLLER_ACCOUNT_INFO_QUERY: &str = r#"
query ControllerAccountInfo {
  me {
    id
    username
    controllers {
      edges {
        node {
          id
          address
        }
      }
    }
  }
}
"#;

#[derive(Debug, Deserialize)]
struct ControllerAccountInfoResponse {
    me: Option<ControllerAccountInfo>,
}

#[derive(Debug, Deserialize)]
struct ControllerAccountInfo {
    id: String,
    username: String,
    controllers: ControllerEdges,
}

#[derive(Debug, Deserialize)]
struct ControllerEdges {
    edges: Option<Vec<Option<ControllerEdge>>>,
}

#[derive(Debug, Deserialize)]
struct ControllerEdge {
    node: Option<ControllerNode>,
}

#[derive(Debug, Deserialize)]
struct ControllerNode {
    id: String,
    address: String,
}

#[derive(Debug, Serialize)]
struct GraphqlRequest<'a, T>
where
    T: Serialize,
{
    query: &'a str,
    variables: T,
}

/// Create a new Catridge Controller account based on session key.
///
/// For now, Controller guarantees that if the provided network is among one of the supported
/// networks, then the Controller account should exist. If it doesn't yet exist, it will
/// automatically be created when a session is created (ie during the session registration stage).
///
/// # Supported networks
///
/// * Starknet mainnet
/// * Starknet sepolia
/// * Slot hosted networks
#[tracing::instrument(name = "create_controller", skip(rpc_url, rpc_provider, contracts))]
pub async fn create_controller(
    // Ideally we can get the url from the provider so we dont have to pass an extra url param here
    rpc_url: Url,
    rpc_provider: CartridgeJsonRpcProvider,
    contracts: &HashMap<String, ContractInfo>,
) -> Result<ControllerAccount> {
    let chain_id = rpc_provider.chain_id().await?;

    trace!(target: "account::controller", "Loading Slot credentials.");
    let credentials = load_or_bootstrap_credentials().await?;
    let username = credentials.account.id;

    // Right now, the Cartridge Controller API ensures that there's always a Controller associated
    // with an account, but that might change in the future.
    let Some(contract_address) = credentials.account.controllers.first().map(|c| c.address) else {
        bail!("No Controller is associated with this account.");
    };

    let policies = collect_policies(contract_address, contracts)?;

    // Check if the session exists, if not create a new one
    let session_details = match slot::session::get(chain_id)? {
        Some(session) => {
            trace!(target: "account::controller", expires_at = %session.session.inner.expires_at, policies = session.session.proved_policies.len(), "Found existing session.");

            // Check if the policies have changed
            let is_equal = is_equal_to_existing(&policies, &session);

            let is_registered = is_session_registered_onchain(
                &rpc_provider,
                session.auth.address,
                chain_id,
                &session,
            )
            .await?;

            if is_equal && is_registered {
                session
            } else {
                trace!(
                    target: "account::controller",
                    new_policies = policies.len(),
                    existing_policies = session.session.requested_policies.len(),
                    is_registered,
                    "Session missing onchain or policies changed. Creating new session."
                );

                let session = slot::session::create(rpc_url.clone(), &policies).await?;
                ensure_session_registered_onchain(
                    &rpc_provider,
                    session.auth.address,
                    chain_id,
                    &session,
                )
                .await?;
                slot::session::store(chain_id, &session)?;
                session
            }
        }

        // Create a new session if not found
        None => {
            trace!(target: "account::controller", %username, chain = format!("{chain_id:#}"), "Creating new session.");
            let session = slot::session::create(rpc_url.clone(), &policies).await?;
            ensure_session_registered_onchain(
                &rpc_provider,
                session.auth.address,
                chain_id,
                &session,
            )
            .await?;
            slot::session::store(chain_id, &session)?;
            session
        }
    };

    Ok(session_details.into_account(rpc_provider))
}

async fn load_or_bootstrap_credentials() -> Result<slot::credential::Credentials> {
    match slot::credential::Credentials::load() {
        Ok(credentials) => Ok(credentials),
        Err(err) if should_bootstrap_credentials(&err) => {
            trace!(
                target: "account::controller",
                error = %err,
                "No valid controller credentials found. Starting inline authorization flow."
            );
            bootstrap_credentials().await?;
            slot::credential::Credentials::load()
                .context("Controller credentials were created but could not be loaded")
                .map_err(Into::into)
        }
        Err(err) => Err(err.into()),
    }
}

fn should_bootstrap_credentials(err: &slot::Error) -> bool {
    matches!(
        err,
        slot::Error::Unauthorized | slot::Error::MalformedCredentials | slot::Error::InvalidOAuth
    )
}

async fn bootstrap_credentials() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .context("Failed to start local callback listener for controller authorization")?;

    let callback_uri = format!(
        "http://127.0.0.1:{}{}",
        listener.local_addr()?.port(),
        CONTROLLER_OAUTH_CALLBACK_PATH
    );

    let mut authorize_url = Url::parse(&slot::vars::get_cartridge_keychain_url())
        .context("Invalid Cartridge keychain URL")?;
    authorize_url.set_path(CONTROLLER_LOGIN_PATH);
    authorize_url.query_pairs_mut().append_pair("callback_uri", &callback_uri);

    println!("Authorize your controller account in browser:\n\n    {}\n", authorize_url);

    slot::browser::open(authorize_url.as_str())?;

    let code = tokio::time::timeout(
        Duration::from_secs(CONTROLLER_OAUTH_TIMEOUT_SECS),
        tokio::task::spawn_blocking(move || wait_for_oauth_code(listener)),
    )
    .await
    .map_err(|_| {
        anyhow!(
            "Timed out waiting for controller authorization callback after {} seconds.",
            CONTROLLER_OAUTH_TIMEOUT_SECS
        )
    })?
    .map_err(|e| anyhow!("Failed to run controller authorization callback listener: {e}"))??;

    let mut api = slot::api::Client::new();
    let token = api.oauth2(&code).await.context("Failed to exchange OAuth code")?;
    api.set_token(token.clone());

    let account_info = fetch_controller_account_info(&api)
        .await
        .context("Failed to load Controller account details after authorization")?;

    let path = slot::credential::Credentials::new(account_info, token)
        .store()
        .context("Failed to store controller credentials")?;

    trace!(
        target: "account::controller",
        path = %path.display(),
        "Controller credentials stored."
    );

    Ok(())
}

async fn fetch_controller_account_info(
    api: &slot::api::Client,
) -> Result<slot::account::AccountInfo> {
    let request =
        GraphqlRequest { query: CONTROLLER_ACCOUNT_INFO_QUERY, variables: serde_json::json!({}) };

    let response: ControllerAccountInfoResponse = api.query(&request).await?;
    let me = response.me.ok_or_else(|| anyhow!("Missing `me` account info in API response"))?;

    let mut controllers = Vec::new();
    for edge in me.controllers.edges.unwrap_or_default().into_iter().flatten() {
        let Some(node) = edge.node else {
            continue;
        };

        let address = Felt::from_str(&node.address)
            .with_context(|| format!("Invalid controller address `{}`", node.address))?;

        controllers.push(slot::account::Controller { id: node.id, address });
    }

    Ok(slot::account::AccountInfo {
        id: me.id,
        username: me.username,
        controllers,
        credentials: Vec::new(),
    })
}

fn wait_for_oauth_code(listener: TcpListener) -> Result<String> {
    let (mut stream, _) =
        listener.accept().context("Failed to accept controller OAuth callback connection")?;

    let mut buffer = [0_u8; 8192];
    let bytes_read =
        stream.read(&mut buffer).context("Failed to read controller OAuth callback request")?;
    if bytes_read == 0 {
        bail!("Controller OAuth callback request was empty.");
    }

    let request = String::from_utf8_lossy(&buffer[..bytes_read]);
    let request_line = request.lines().next().unwrap_or_default();
    let target = request_line
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| anyhow!("Invalid callback request line: `{request_line}`"))?;

    let Some(code) = extract_oauth_code(target) else {
        write_http_response(
            &mut stream,
            "400 Bad Request",
            "Missing authorization code. You can close this tab and retry.",
        )?;
        bail!("Controller OAuth callback does not contain `code` query parameter.");
    };

    write_http_response(
        &mut stream,
        "200 OK",
        "Controller authorization received. You can close this tab and return to sozo.",
    )?;

    Ok(code)
}

fn extract_oauth_code(target: &str) -> Option<String> {
    let callback_url = Url::parse(&format!("http://localhost{target}")).ok()?;
    if callback_url.path() != CONTROLLER_OAUTH_CALLBACK_PATH {
        return None;
    }

    callback_url.query_pairs().find_map(|(key, value)| (key == "code").then(|| value.into_owned()))
}

fn write_http_response(stream: &mut TcpStream, status: &str, body: &str) -> Result<()> {
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: \
         {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes())?;
    stream.flush()?;
    Ok(())
}

// Check if the new policies are equal to the ones in the existing session
//
// This function would compute the merkle root of the new policies and compare it with the root in
// the existing SessionMetadata.
fn is_equal_to_existing(new_policies: &[PolicyMethod], session_info: &FullSessionInfo) -> bool {
    let new_policies = new_policies
        .iter()
        .map(|p| {
            Policy::Call(CallPolicy {
                authorized: Some(true),
                contract_address: p.target,
                selector: get_selector_from_name(&p.method).expect("valid selector"),
            })
        })
        .collect::<Vec<Policy>>();

    // Copied from Session::new
    let hashes = new_policies.iter().map(Policy::as_merkle_leaf).collect::<Vec<Felt>>();

    let new_policies = new_policies
        .into_iter()
        .enumerate()
        .map(|(i, policy)| ProvedPolicy {
            policy,
            proof: MerkleTree::compute_proof(hashes.clone(), i),
        })
        .collect::<Vec<ProvedPolicy>>();

    let new_policies_root = MerkleTree::compute_root(hashes[0], new_policies[0].proof.clone());
    new_policies_root == session_info.session.inner.allowed_policies_root
}

async fn is_session_registered_onchain(
    provider: &CartridgeJsonRpcProvider,
    controller_address: Felt,
    chain_id: Felt,
    session: &FullSessionInfo,
) -> Result<bool> {
    let session_hash = session.session.inner.get_message_hash_rev_1(chain_id, controller_address);

    let call = FunctionCall {
        contract_address: controller_address,
        entry_point_selector: get_selector_from_name("is_session_registered")
            .context("Failed to resolve selector for `is_session_registered`")?,
        calldata: vec![session_hash],
    };

    let result = provider.call(call, BlockId::Tag(BlockTag::Latest)).await?;
    Ok(result.first().is_some_and(|v| *v != Felt::ZERO))
}

async fn ensure_session_registered_onchain(
    provider: &CartridgeJsonRpcProvider,
    controller_address: Felt,
    chain_id: Felt,
    session: &FullSessionInfo,
) -> Result<()> {
    if is_session_registered_onchain(provider, controller_address, chain_id, session).await? {
        return Ok(());
    }

    let timeout = Duration::from_secs(CONTROLLER_SESSION_REGISTRATION_TIMEOUT_SECS);
    let poll = Duration::from_millis(CONTROLLER_SESSION_REGISTRATION_POLL_MS);
    let started = std::time::Instant::now();

    while started.elapsed() < timeout {
        tokio::time::sleep(poll).await;

        if is_session_registered_onchain(provider, controller_address, chain_id, session).await? {
            return Ok(());
        }
    }

    bail!(
        "Controller session was created locally but is not registered onchain yet (timeout: {}s). \
         Please retry `sozo controller session create`.",
        CONTROLLER_SESSION_REGISTRATION_TIMEOUT_SECS
    );
}

/// Policies are the building block of a session key. It's what defines what methods are allowed for
/// an external signer to execute using the session key.
///
/// This function collect all the contracts' methods in the current project according to the
/// project's base manifest ( `/manifests/<profile>/base` ) and convert them into policies.
fn collect_policies(
    user_address: Felt,
    contracts: &HashMap<String, ContractInfo>,
) -> Result<Vec<PolicyMethod>> {
    let policies = collect_policies_from_contracts(user_address, contracts)?;
    trace!(target: "account::controller", policies_count = policies.len(), "Extracted policies from project.");
    Ok(policies)
}

fn collect_policies_from_contracts(
    user_address: Felt,
    contracts: &HashMap<String, ContractInfo>,
) -> Result<Vec<PolicyMethod>> {
    let mut policies: Vec<PolicyMethod> = Vec::new();

    for (tag, info) in contracts {
        for e in &info.entrypoints {
            let policy = PolicyMethod { target: info.address, method: e.clone() };
            trace!(target: "account::controller", tag, target = format!("{:#x}", policy.target), method = %policy.method, "Adding policy");
            policies.push(policy);
        }
    }

    // special policy for sending declare tx
    // corresponds to [account_sdk::account::DECLARATION_SELECTOR]
    let method = "__declare_transaction__".to_string();
    policies.push(PolicyMethod { target: user_address, method });
    trace!(target: "account::controller", "Adding declare transaction policy");

    // for deploying using udc
    let method = "deployContract".to_string();
    const UDC_ADDRESS: Felt =
        felt!("0x041a78e741e5af2fec34b695679bc6891742439f7afb8484ecd7766661ad02bf");
    policies.push(PolicyMethod { target: UDC_ADDRESS, method });
    trace!(target: "account::controller", "Adding UDC deployment policy");

    // Keep a deterministic policy order so session root comparison remains stable across runs.
    policies.sort_by(|a, b| a.target.cmp(&b.target).then_with(|| a.method.cmp(&b.method)));

    Ok(policies)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use dojo_test_utils::setup::TestSetup;
    use dojo_world::contracts::ContractInfo;
    use scarb_interop::Profile;
    use scarb_metadata_ext::MetadataDojoExt;
    use starknet::macros::felt;

    use super::{
        PolicyMethod, collect_policies, collect_policies_from_contracts, extract_oauth_code,
    };

    #[test]
    fn collect_policies_from_project() {
        let setup = TestSetup::from_examples("../../crates/dojo/core", "../../examples/");
        let scarb_metadata = setup.load_metadata("spawn-and-move", Profile::DEV);

        let manifest =
            scarb_metadata.read_dojo_manifest_profile().expect("Failed to read manifest").unwrap();
        let contracts: HashMap<String, ContractInfo> = (&manifest).into();
        let world_address = contracts.get("world").unwrap().address;
        let actions = contracts.get("ns-actions").unwrap();
        let actions_address = actions.address;
        let world = contracts.get("world").unwrap();

        let user_addr = felt!("0x2af9427c5a277474c079a1283c880ee8a6f0f8fbf73ce969c08d88befec1bba");

        let policies = collect_policies(user_addr, &contracts).unwrap();

        // Should include user systems.
        assert!(
            policies.contains(&PolicyMethod { target: actions_address, method: "spawn".into() })
        );
        assert!(
            policies.contains(&PolicyMethod { target: actions_address, method: "move".into() })
        );

        // Should include world contract policies.
        assert!(
            policies.iter().any(|p| p.target == world_address),
            "world entrypoints should be included in session policies"
        );

        // World methods from manifest should be part of policies.
        for method in &world.entrypoints {
            assert!(
                policies.contains(&PolicyMethod { target: world_address, method: method.clone() })
            );
        }

        // Should keep required meta policies.
        assert!(
            policies.contains(&PolicyMethod {
                target: user_addr,
                method: "__declare_transaction__".into(),
            }),
            "declare policy is missing"
        );
        assert!(
            policies.contains(&PolicyMethod {
                target: felt!("0x041a78e741e5af2fec34b695679bc6891742439f7afb8484ecd7766661ad02bf"),
                method: "deployContract".into(),
            }),
            "UDC deployment policy is missing"
        );
    }

    #[test]
    fn collect_policies_includes_world_and_upgrade() {
        let user_addr = felt!("0x123");
        let world_addr = felt!("0x456");
        let actions_addr = felt!("0x789");

        let mut contracts = HashMap::new();
        contracts.insert(
            "world".to_string(),
            ContractInfo {
                tag_or_name: "world".to_string(),
                address: world_addr,
                entrypoints: vec!["register_model".into(), "set_entity".into()],
            },
        );
        contracts.insert(
            "ns-actions".to_string(),
            ContractInfo {
                tag_or_name: "ns-actions".to_string(),
                address: actions_addr,
                entrypoints: vec!["spawn".into(), "move".into(), "upgrade".into()],
            },
        );

        let policies = collect_policies(user_addr, &contracts).unwrap();

        assert!(
            policies
                .contains(&PolicyMethod { target: world_addr, method: "register_model".into() })
        );
        assert!(
            policies.contains(&PolicyMethod { target: world_addr, method: "set_entity".into() })
        );
        assert!(policies.contains(&PolicyMethod { target: actions_addr, method: "spawn".into() }));
        assert!(policies.contains(&PolicyMethod { target: actions_addr, method: "move".into() }));
        assert!(
            policies.contains(&PolicyMethod { target: actions_addr, method: "upgrade".into() })
        );
    }

    #[test]
    fn collect_policies_has_stable_order() {
        let user_addr = felt!("0x123");
        let a_addr = felt!("0x2");
        let b_addr = felt!("0x1");

        let mut contracts_a = HashMap::new();
        contracts_a.insert(
            "a".to_string(),
            ContractInfo {
                tag_or_name: "a".to_string(),
                address: a_addr,
                entrypoints: vec!["z".into(), "a".into()],
            },
        );
        contracts_a.insert(
            "b".to_string(),
            ContractInfo {
                tag_or_name: "b".to_string(),
                address: b_addr,
                entrypoints: vec!["m".into()],
            },
        );

        let mut contracts_b = HashMap::new();
        contracts_b.insert(
            "b".to_string(),
            ContractInfo {
                tag_or_name: "b".to_string(),
                address: b_addr,
                entrypoints: vec!["m".into()],
            },
        );
        contracts_b.insert(
            "a".to_string(),
            ContractInfo {
                tag_or_name: "a".to_string(),
                address: a_addr,
                entrypoints: vec!["z".into(), "a".into()],
            },
        );

        let policies_a = collect_policies_from_contracts(user_addr, &contracts_a).unwrap();
        let policies_b = collect_policies_from_contracts(user_addr, &contracts_b).unwrap();

        assert_eq!(policies_a, policies_b);
    }

    #[test]
    fn extract_oauth_code_from_callback_target() {
        let code = extract_oauth_code("/callback?code=abc123&state=xyz");
        assert_eq!(code.as_deref(), Some("abc123"));
    }

    #[test]
    fn extract_oauth_code_decodes_url_encoded_value() {
        let code = extract_oauth_code("/callback?code=abc%2F123");
        assert_eq!(code.as_deref(), Some("abc/123"));
    }

    #[test]
    fn extract_oauth_code_rejects_non_callback_target() {
        assert_eq!(extract_oauth_code("/not-callback?code=abc123"), None);
    }
}
