use std::cmp::Reverse;
use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use base64::{Engine as _, engine::general_purpose};
use cainome_cairo_serde::NonZero;
use dojo_world::contracts::contract_info::ContractInfo;
use serde::{Deserialize, Serialize};
use slot::account_sdk::abigen::controller::{Signer as ControllerSigner, StarknetSigner};
use slot::account_sdk::account::session::account::SessionAccount;
use slot::account_sdk::account::session::hash::Session;
use slot::account_sdk::account::session::policy::{CallPolicy, Policy};
use slot::account_sdk::hash::MessageHashRev1;
use slot::account_sdk::provider::CartridgeJsonRpcProvider;
use slot::session::{FullSessionInfo, PolicyMethod};
use starknet::core::types::Felt;
use starknet::core::utils::get_selector_from_name;
use starknet::macros::felt;
use starknet::providers::Provider;
use starknet::providers::jsonrpc::{HttpTransport, JsonRpcClient};
use starknet::signers::SigningKey;
use tracing::trace;
use url::Url;

#[allow(missing_debug_implementations)]
pub type ControllerAccount = SessionAccount;

const CONTROLLER_OAUTH_TIMEOUT_SECS: u64 = 300;
const CONTROLLER_OAUTH_CALLBACK_PATH: &str = "/callback";
const CONTROLLER_LOGIN_PATH: &str = "/slot";
const CONTROLLER_SESSION_CREATION_PATH: &str = "/session";
const CONTROLLER_SHORTENER_PATH: &str = "/s";
const CONTROLLER_SESSION_TIMEOUT_SECS: u64 = 300;
const MULTI_SESSION_FILE_INFIX: &str = "-session-";
const SOZO_PROFILE_ENV_VAR: &str = "SOZO_PROFILE";
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

#[derive(Debug, Serialize)]
struct ShortUrlRequest<'a> {
    url: &'a str,
}

#[derive(Debug, Deserialize)]
struct ShortUrlResponse {
    url: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ControllerSessionResponse {
    username: String,
    address: Felt,
    owner_guid: Felt,
    expires_at: String,
    transaction_hash: Option<Felt>,
    #[serde(default)]
    already_registered: bool,
    allowed_policies_root: Option<Felt>,
    metadata_hash: Option<Felt>,
    session_key_guid: Option<Felt>,
    guardian_key_guid: Option<Felt>,
}

fn session_user_dir(username: &str) -> PathBuf {
    slot::utils::config_dir().join(username)
}

fn fnv1a64(input: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in input {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn discover_project_root() -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    for ancestor in cwd.ancestors() {
        if ancestor.join("Scarb.toml").exists() {
            return ancestor.to_path_buf();
        }
    }
    cwd
}

pub(crate) fn current_session_context_hash() -> String {
    let profile = std::env::var(SOZO_PROFILE_ENV_VAR)
        .or_else(|_| std::env::var("SCARB_PROFILE"))
        .unwrap_or_else(|_| "dev".to_string());
    let project_root = discover_project_root();
    let context_raw = format!("project={}|profile={}", project_root.display(), profile);
    format!("{:016x}", fnv1a64(context_raw.as_bytes()))
}

fn multi_session_file_path(
    username: &str,
    chain_id: Felt,
    context_hash: &str,
    policy_root: Felt,
) -> PathBuf {
    session_user_dir(username).join(format!(
        "{chain_id:#x}{MULTI_SESSION_FILE_INFIX}{context_hash}-{policy_root:064x}.json"
    ))
}

fn load_multi_sessions_for_chain(
    username: &str,
    chain_id: Felt,
    context_hash: &str,
) -> Result<Vec<FullSessionInfo>> {
    let user_dir = session_user_dir(username);
    if !user_dir.exists() {
        return Ok(Vec::new());
    }

    let chain_prefix = format!("{chain_id:#x}{MULTI_SESSION_FILE_INFIX}{context_hash}-");
    let mut sessions = Vec::new();

    for entry in fs::read_dir(&user_dir).context("Failed to read controller session directory")? {
        let path = entry?.path();
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };

        if !file_name.starts_with(&chain_prefix) || !file_name.ends_with(".json") {
            continue;
        }

        let contents = match fs::read_to_string(&path) {
            Ok(contents) => contents,
            Err(err) => {
                trace!(
                    target: "account::controller",
                    path = %path.display(),
                    error = %err,
                    "Failed to read stored multi-session file, skipping."
                );
                continue;
            }
        };

        match serde_json::from_str::<FullSessionInfo>(&contents) {
            Ok(session) if session.chain_id == chain_id => sessions.push(session),
            Ok(_) => {
                trace!(
                    target: "account::controller",
                    path = %path.display(),
                    "Skipping multi-session file with mismatched chain id."
                );
            }
            Err(err) => {
                trace!(
                    target: "account::controller",
                    path = %path.display(),
                    error = %err,
                    "Failed to parse stored multi-session file, skipping."
                );
            }
        }
    }

    Ok(sessions)
}

fn find_matching_stored_session(
    username: &str,
    chain_id: Felt,
    context_hash: &str,
    policies: &[PolicyMethod],
) -> Result<Option<FullSessionInfo>> {
    let mut candidates = load_multi_sessions_for_chain(username, chain_id, context_hash)?;
    if candidates.is_empty() {
        // Backward-compatible fallback for users that only have the legacy single-session file.
        if let Some(session) = slot::session::get(chain_id)? {
            candidates.push(session);
        }
    }

    let mut dedup = BTreeSet::new();
    candidates.retain(|session| {
        let key = format!(
            "{:#x}:{:#x}:{:#x}:{}",
            session.auth.address,
            session.auth.owner_guid,
            session.session.inner.session_key_guid,
            session.session.inner.expires_at
        );
        dedup.insert(key)
    });

    let mut matching = candidates
        .into_iter()
        .filter(|session| is_equal_to_existing(policies, session))
        .collect::<Vec<_>>();

    matching.sort_by_key(|session| Reverse(session.session.inner.expires_at));
    Ok(matching.into_iter().next())
}

fn persist_session_files(
    chain_id: Felt,
    context_hash: &str,
    session: &FullSessionInfo,
) -> Result<()> {
    slot::session::store(chain_id, session)?;

    let path = multi_session_file_path(
        &session.auth.username,
        chain_id,
        context_hash,
        session.session.inner.allowed_policies_root,
    );
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context("Failed to create controller session directory")?;
    }

    let contents =
        serde_json::to_string_pretty(session).context("Failed to serialize controller session")?;
    fs::write(&path, contents).context("Failed to persist controller multi-session file")?;

    Ok(())
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
    let context_hash = current_session_context_hash();

    // Resolve the best stored session for this policy set and chain.
    // This allows multiple project sessions to coexist on the same account/network.
    let session_details =
        match find_matching_stored_session(&username, chain_id, &context_hash, &policies)? {
            Some(session) if !session.session.is_expired() => {
                trace!(
                    target: "account::controller",
                    context_hash = %context_hash,
                    expires_at = %session.session.inner.expires_at,
                    policies = session.session.proved_policies.len(),
                    "Reusing matching stored session."
                );
                session
            }
            Some(session) => {
                trace!(
                    target: "account::controller",
                    context_hash = %context_hash,
                    expires_at = %session.session.inner.expires_at,
                    "Matching stored session is expired. Creating a new session."
                );
                create_session_with_short_url(
                    rpc_url.clone(),
                    chain_id,
                    contract_address,
                    None,
                    &policies,
                )
                .await?
            }
            None => {
                trace!(
                    target: "account::controller",
                    %username,
                    context_hash = %context_hash,
                    chain = format!("{chain_id:#}"),
                    "No matching stored session found. Creating new session."
                );
                create_session_with_short_url(
                    rpc_url.clone(),
                    chain_id,
                    contract_address,
                    None,
                    &policies,
                )
                .await?
            }
        };

    persist_session_files(chain_id, &context_hash, &session_details)?;

    Ok(session_details.into_account(rpc_provider))
}

async fn create_session_with_short_url(
    rpc_url: Url,
    chain_id: Felt,
    expected_controller_address: Felt,
    existing_session: Option<&FullSessionInfo>,
    policies: &[PolicyMethod],
) -> Result<FullSessionInfo> {
    let signer = SigningKey::from_random();
    let pubkey = signer.verifying_key().scalar();

    let credentials = slot::credential::Credentials::load()?;
    let username = credentials.account.id;

    let response =
        create_user_session_with_short_url(pubkey, &username, rpc_url.clone(), policies).await?;
    trace!(
        target: "account::controller",
        already_registered = response.already_registered,
        transaction_hash = ?response.transaction_hash,
        "Received controller session callback response."
    );
    if response.address != expected_controller_address {
        bail!(
            "Controller session callback address mismatch. expected={:#x}, callback={:#x}",
            expected_controller_address,
            response.address
        );
    }

    let expires_at = response.expires_at.parse::<u64>().map_err(|e| anyhow!(e))?;
    let mut session = build_session_from_policies(policies, expires_at, &signer, &response)?;
    let mut local_hash =
        session.inner.get_message_hash_rev_1(chain_id, expected_controller_address);

    // Trust on-chain registration status instead of GraphQL replication state.
    let mut local_hash_registered = is_session_hash_registered_onchain(
        &rpc_url,
        expected_controller_address,
        response.owner_guid,
        local_hash,
    )
    .await?;

    if !local_hash_registered {
        // If controller reports already-registered, prefer reusing the currently stored session
        // when it is still registered on-chain.
        if response.already_registered {
            if let Some(existing) = existing_session {
                let existing_hash = existing
                    .session
                    .inner
                    .get_message_hash_rev_1(chain_id, expected_controller_address);
                if is_session_hash_registered_onchain(
                    &rpc_url,
                    expected_controller_address,
                    response.owner_guid,
                    existing_hash,
                )
                .await?
                {
                    trace!(
                        target: "account::controller",
                        existing_hash = format!("{:#x}", existing_hash),
                        local_hash = format!("{:#x}", local_hash),
                        "Reusing previously stored registered session after callback hash mismatch."
                    );
                    return Ok(existing.clone());
                }
            }
        }

        // Try alternate deterministic policy orderings to match keychain canonicalization.
        for candidate in alternate_policy_orders(policies) {
            let candidate_session =
                build_session_from_policies(&candidate, expires_at, &signer, &response)?;
            let candidate_hash = candidate_session
                .inner
                .get_message_hash_rev_1(chain_id, expected_controller_address);

            if candidate_hash == local_hash {
                continue;
            }

            if is_session_hash_registered_onchain(
                &rpc_url,
                expected_controller_address,
                response.owner_guid,
                candidate_hash,
            )
            .await?
            {
                trace!(
                    target: "account::controller",
                    previous_hash = format!("{:#x}", local_hash),
                    matched_hash = format!("{:#x}", candidate_hash),
                    "Recovered registered controller session hash using alternate policy ordering."
                );
                session = candidate_session;
                local_hash = candidate_hash;
                local_hash_registered = true;
                break;
            }
        }
    }

    if !local_hash_registered {
        bail!(
            "Registered session hash mismatch. local={:#x}, controller={:#x}, owner_guid={:#x}. The session was not found on-chain for this owner/session tuple.",
            local_hash,
            expected_controller_address,
            response.owner_guid
        );
    }

    let auth = slot::session::SessionAuth {
        address: response.address,
        username: response.username,
        owner_guid: response.owner_guid,
        signer: signer.secret_scalar(),
    };

    Ok(FullSessionInfo { auth, session, chain_id })
}

fn build_session_from_policies(
    policies: &[PolicyMethod],
    expires_at: u64,
    signer: &SigningKey,
    response: &ControllerSessionResponse,
) -> Result<Session> {
    let methods = policies
        .iter()
        .map(|p| -> Result<Policy> {
            let selector = get_selector_from_name(&p.method)?;
            Ok(Policy::Call(CallPolicy {
                contract_address: p.target,
                selector,
                authorized: Some(true),
            }))
        })
        .collect::<Result<Vec<_>>>()?;

    let mut session = Session::new(
        methods,
        expires_at,
        &ControllerSigner::Starknet(StarknetSigner {
            pubkey: NonZero::new(signer.verifying_key().scalar())
                .expect("public key scalar should not be zero"),
        }),
        Felt::ZERO,
    )?;

    apply_session_response_overrides(&mut session, response)?;
    Ok(session)
}

fn alternate_policy_orders(policies: &[PolicyMethod]) -> Vec<Vec<PolicyMethod>> {
    let mut candidates = Vec::new();

    let mut by_unpadded_address_then_method = policies.to_vec();
    by_unpadded_address_then_method.sort_by(|a, b| {
        format!("{:#x}", a.target)
            .to_ascii_lowercase()
            .cmp(&format!("{:#x}", b.target).to_ascii_lowercase())
            .then_with(|| a.method.cmp(&b.method))
    });
    candidates.push(by_unpadded_address_then_method);

    let mut by_address_then_method_casefold = policies.to_vec();
    by_address_then_method_casefold.sort_by(|a, b| {
        format!("0x{:064x}", a.target).cmp(&format!("0x{:064x}", b.target)).then_with(|| {
            a.method
                .to_ascii_lowercase()
                .cmp(&b.method.to_ascii_lowercase())
                .then_with(|| a.method.cmp(&b.method))
        })
    });
    candidates.push(by_address_then_method_casefold);

    let mut by_method_then_address = policies.to_vec();
    by_method_then_address.sort_by(|a, b| {
        a.method
            .cmp(&b.method)
            .then_with(|| format!("0x{:064x}", a.target).cmp(&format!("0x{:064x}", b.target)))
    });
    candidates.push(by_method_then_address);

    // Keep first occurrence only while preserving insertion order.
    let mut unique = Vec::new();
    for candidate in candidates {
        if !unique.contains(&candidate) {
            unique.push(candidate);
        }
    }

    unique
}

async fn create_user_session_with_short_url(
    public_key: Felt,
    username: &str,
    rpc_url: Url,
    policies: &[PolicyMethod],
) -> Result<ControllerSessionResponse> {
    let listener = TcpListener::bind("localhost:0")
        .context("Failed to start local callback listener for controller session authorization")?;
    let callback_uri = format!(
        "http://localhost:{}{}",
        listener.local_addr()?.port(),
        CONTROLLER_OAUTH_CALLBACK_PATH
    );

    let authorize_url = build_session_creation_url(
        public_key,
        username,
        rpc_url.as_str(),
        policies,
        &callback_uri,
    )?;
    let open_url = shorten_session_authorize_url(&authorize_url).await.unwrap_or_else(|err| {
        trace!(
            target: "account::controller",
            error = %err,
            "Failed to shorten controller session URL, falling back to full URL."
        );
        authorize_url.clone()
    });

    println!("Authorize your controller session in browser:\n\n    {}\n", open_url);
    slot::browser::open(open_url.as_str())?;

    let payload = tokio::time::timeout(
        Duration::from_secs(CONTROLLER_SESSION_TIMEOUT_SECS),
        tokio::task::spawn_blocking(move || wait_for_session_payload(listener)),
    )
    .await
    .map_err(|_| {
        anyhow!(
            "Timed out waiting for controller session callback after {} seconds.",
            CONTROLLER_SESSION_TIMEOUT_SECS
        )
    })?
    .map_err(|e| anyhow!("Failed to run controller session callback listener: {e}"))??;

    parse_session_creation_response(&payload)
}

fn build_session_creation_url(
    public_key: Felt,
    username: &str,
    rpc_url: &str,
    policies: &[PolicyMethod],
    callback_uri: &str,
) -> Result<Url> {
    let encoded_policies = policies
        .iter()
        .map(serde_json::to_string)
        .map(|p| Ok(url_encode_query_component(&p?)))
        .collect::<Result<Vec<_>, serde_json::Error>>()?
        .join(",");

    let params = format!(
        "username={username}&public_key={public_key}&rpc_url={rpc_url}&policies=[{encoded_policies}]"
    );
    let host = slot::vars::get_cartridge_keychain_url();
    let mut url = Url::parse(&format!("{host}{CONTROLLER_SESSION_CREATION_PATH}?{params}"))
        .context("Invalid Cartridge keychain URL")?;
    url.query_pairs_mut().append_pair("callback_uri", callback_uri);
    Ok(url)
}

fn url_encode_query_component(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.as_bytes()).collect()
}

async fn shorten_session_authorize_url(authorize_url: &Url) -> Result<Url> {
    let base = slot::vars::get_cartridge_api_url();
    let endpoint = format!(
        "{}/{}",
        base.trim_end_matches('/'),
        CONTROLLER_SHORTENER_PATH.trim_start_matches('/')
    );

    let response = reqwest::Client::new()
        .post(endpoint)
        .json(&ShortUrlRequest { url: authorize_url.as_str() })
        .send()
        .await
        .context("Failed to call Cartridge short URL endpoint")?;

    if !response.status().is_success() {
        bail!("Cartridge short URL endpoint returned HTTP {}", response.status());
    }

    let body: ShortUrlResponse =
        response.json().await.context("Failed to decode Cartridge short URL response body")?;
    Url::parse(&body.url).context("Invalid short URL returned by Cartridge API")
}

async fn is_session_hash_registered_onchain(
    rpc_url: &Url,
    controller_address: Felt,
    owner_guid: Felt,
    session_hash: Felt,
) -> Result<bool> {
    let provider = JsonRpcClient::new(HttpTransport::new(rpc_url.clone()));
    let reader =
        slot::account_sdk::abigen::controller::ControllerReader::new(controller_address, provider);

    // Check both owner GUID and controller address. Different deployments may use one or the other
    // as the authorizer key for `is_session_registered`.
    let mut authorizers = vec![owner_guid, controller_address];
    authorizers.dedup();

    let mut successful_checks = 0usize;
    let mut last_error = None;

    for authorizer in authorizers {
        match reader.is_session_registered(&session_hash, &authorizer).call().await {
            Ok(true) => return Ok(true),
            Ok(false) => {
                successful_checks += 1;
            }
            Err(err) => {
                trace!(
                    target: "account::controller",
                    authorizer = format!("{:#x}", authorizer),
                    error = %err,
                    "Failed to query session registration for authorizer."
                );
                last_error = Some(err);
            }
        }
    }

    if successful_checks == 0 {
        if let Some(err) = last_error {
            return Err(anyhow!(
                "Failed to query session registration status on controller contract: {err}"
            ));
        }
    }

    Ok(false)
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
    let listener = TcpListener::bind("localhost:0")
        .context("Failed to start local callback listener for controller authorization")?;

    let callback_uri = format!(
        "http://localhost:{}{}",
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

fn wait_for_session_payload(listener: TcpListener) -> Result<String> {
    loop {
        let (mut stream, _) =
            listener.accept().context("Failed to accept controller session callback connection")?;
        let request = read_http_request(&mut stream)?;

        let Some(headers_end) = request.windows(4).position(|window| window == b"\r\n\r\n") else {
            write_http_response(
                &mut stream,
                "400 Bad Request",
                "Malformed callback request. You can close this tab and retry.",
            )?;
            continue;
        };

        let head = String::from_utf8_lossy(&request[..headers_end]);
        let request_line = head.lines().next().unwrap_or_default();
        let mut request_line_parts = request_line.split_whitespace();
        let method = request_line_parts.next().unwrap_or_default();
        let Some(target) = request_line_parts.next() else {
            write_http_response(
                &mut stream,
                "400 Bad Request",
                "Malformed callback request line. You can close this tab and retry.",
            )?;
            continue;
        };

        let callback_url = Url::parse(&format!("http://localhost{target}"))
            .context("Failed to parse callback target URL")?;
        if callback_url.path() != CONTROLLER_OAUTH_CALLBACK_PATH {
            write_http_response(
                &mut stream,
                "400 Bad Request",
                "Invalid callback path. You can close this tab and retry.",
            )?;
            continue;
        }

        if method.eq_ignore_ascii_case("OPTIONS") {
            write_http_response(&mut stream, "204 No Content", "")?;
            continue;
        }

        if !method.eq_ignore_ascii_case("POST") {
            write_http_response(
                &mut stream,
                "405 Method Not Allowed",
                "Unsupported callback method. You can close this tab and retry.",
            )?;
            continue;
        }

        let content_length = head
            .lines()
            .find_map(|line| {
                let (key, value) = line.split_once(':')?;
                key.eq_ignore_ascii_case("content-length").then(|| value.trim().parse::<usize>())
            })
            .transpose()
            .context("Invalid `content-length` header in controller session callback")?
            .unwrap_or_default();

        let body_start = headers_end + 4;
        if request.len() < body_start + content_length {
            write_http_response(
                &mut stream,
                "400 Bad Request",
                "Incomplete callback payload. You can close this tab and retry.",
            )?;
            continue;
        }

        let body_bytes = &request[body_start..body_start + content_length];
        let body = String::from_utf8(body_bytes.to_vec())
            .context("Controller session callback body is not valid UTF-8")?;
        let body = body.trim();
        let payload = serde_json::from_str::<String>(body).unwrap_or_else(|_| body.to_string());

        if payload.is_empty() {
            write_http_response(
                &mut stream,
                "400 Bad Request",
                "Missing session payload. You can close this tab and retry.",
            )?;
            continue;
        }

        write_http_response(
            &mut stream,
            "200 OK",
            "Controller session received. You can close this tab and return to sozo.",
        )?;

        return Ok(payload);
    }
}

fn parse_session_creation_response(payload: &str) -> Result<ControllerSessionResponse> {
    if let Ok(response) = parse_session_response_encoded(payload) {
        return Ok(response);
    }

    serde_json::from_str(payload)
        .context("Failed to decode controller session callback payload as session JSON.")
}

fn parse_session_response_encoded(encoded: &str) -> Result<ControllerSessionResponse> {
    let bytes = general_purpose::STANDARD_NO_PAD
        .decode(encoded)
        .context("Failed to decode base64 session callback payload")?;
    let decoded =
        String::from_utf8(bytes).context("Session callback payload is not valid UTF-8")?;
    serde_json::from_str(&decoded).context("Failed to decode session callback JSON payload")
}

fn apply_session_response_overrides(
    session: &mut Session,
    response: &ControllerSessionResponse,
) -> Result<()> {
    if let Some(session_key_guid) = response.session_key_guid {
        if session_key_guid != session.inner.session_key_guid {
            bail!(
                "Controller returned a session key guid that does not match the generated session signer."
            );
        }
        session.inner.session_key_guid = session_key_guid;
    }

    if let Some(allowed_policies_root) = response.allowed_policies_root {
        if allowed_policies_root != session.inner.allowed_policies_root {
            bail!(
                "Controller returned a policy root that differs from local policy hashing. Check policy ordering."
            );
        }
        session.inner.allowed_policies_root = allowed_policies_root;
    }

    if let Some(metadata_hash) = response.metadata_hash {
        session.inner.metadata_hash = metadata_hash;
    }

    if let Some(guardian_key_guid) = response.guardian_key_guid {
        session.inner.guardian_key_guid = guardian_key_guid;
    }

    Ok(())
}

fn read_http_request(stream: &mut TcpStream) -> Result<Vec<u8>> {
    const MAX_REQUEST_SIZE: usize = 1024 * 1024;

    let mut request = Vec::with_capacity(8192);
    let mut chunk = [0_u8; 8192];

    loop {
        let bytes_read = stream
            .read(&mut chunk)
            .context("Failed to read controller session callback request")?;
        if bytes_read == 0 {
            break;
        }

        request.extend_from_slice(&chunk[..bytes_read]);
        if request.len() > MAX_REQUEST_SIZE {
            bail!("Controller session callback request is too large.");
        }

        if let Some(headers_end) = request.windows(4).position(|window| window == b"\r\n\r\n") {
            let headers = String::from_utf8_lossy(&request[..headers_end]);
            let content_length = headers
                .lines()
                .find_map(|line| {
                    let (key, value) = line.split_once(':')?;
                    key.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>())
                })
                .transpose()
                .context("Invalid `content-length` header in controller session callback")?
                .unwrap_or_default();

            let expected_len = headers_end + 4 + content_length;
            if request.len() >= expected_len {
                break;
            }
        }
    }

    if request.is_empty() {
        bail!("Controller session callback request was empty.");
    }

    Ok(request)
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
         {}\r\nConnection: close\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: POST, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type\r\n\r\n{body}",
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
    // Compare by canonical call policy content only (contract+selector), ignoring ordering,
    // duplicates, and authorized toggles.
    let new_calls = {
        let mut set = BTreeSet::new();
        for policy in new_policies {
            let Ok(selector) = get_selector_from_name(&policy.method) else {
                return false;
            };
            set.insert((format!("0x{:064x}", policy.target), format!("0x{:064x}", selector)));
        }
        set
    };

    let existing_calls = session_info
        .session
        .requested_policies
        .iter()
        .filter_map(|policy| match policy {
            Policy::Call(call) => Some((
                format!("0x{:064x}", call.contract_address),
                format!("0x{:064x}", call.selector),
            )),
            _ => None,
        })
        .collect::<BTreeSet<_>>();

    new_calls == existing_calls
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

    // Keep canonical ordering aligned with controller/keychain sorting:
    // normalized lowercase padded hex address, then method name.
    policies.sort_by(|a, b| {
        format!("0x{:064x}", a.target)
            .cmp(&format!("0x{:064x}", b.target))
            .then_with(|| a.method.cmp(&b.method))
    });

    Ok(policies)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use base64::{Engine as _, engine::general_purpose};
    use cainome_cairo_serde::NonZero;
    use dojo_test_utils::setup::TestSetup;
    use dojo_world::contracts::ContractInfo;
    use scarb_interop::Profile;
    use scarb_metadata_ext::MetadataDojoExt;
    use slot::account_sdk::abigen::controller::{Signer as ControllerSigner, StarknetSigner};
    use slot::account_sdk::account::session::hash::Session;
    use slot::account_sdk::account::session::policy::{CallPolicy, Policy, TypedDataPolicy};
    use slot::session::{FullSessionInfo, SessionAuth};
    use starknet::core::types::Felt;
    use starknet::core::utils::get_selector_from_name;
    use starknet::macros::felt;
    use starknet::signers::SigningKey;

    use super::{
        PolicyMethod, alternate_policy_orders, collect_policies, collect_policies_from_contracts,
        extract_oauth_code, is_equal_to_existing, parse_session_creation_response,
    };

    fn session_with_requested_policies(requested_policies: Vec<Policy>) -> FullSessionInfo {
        let signer = SigningKey::from_secret_scalar(felt!("0x12345"));
        let session = Session::new(
            requested_policies,
            4_102_444_800,
            &ControllerSigner::Starknet(StarknetSigner {
                pubkey: NonZero::new(signer.verifying_key().scalar())
                    .expect("public key scalar should not be zero"),
            }),
            Felt::ZERO,
        )
        .expect("session should build");

        FullSessionInfo {
            chain_id: felt!("0x534e5f5345504f4c4941"),
            auth: SessionAuth {
                username: "alice".into(),
                address: felt!("0xabc"),
                owner_guid: felt!("0xdef"),
                signer: signer.secret_scalar(),
            },
            session,
        }
    }

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
    fn collect_policies_uses_controller_canonical_address_sort() {
        let user_addr = felt!("0x123");
        let addr_2 = felt!("0x2");
        let addr_10 = felt!("0x10");

        let mut contracts = HashMap::new();
        contracts.insert(
            "two".to_string(),
            ContractInfo {
                tag_or_name: "two".to_string(),
                address: addr_2,
                entrypoints: vec!["exec".into()],
            },
        );
        contracts.insert(
            "ten".to_string(),
            ContractInfo {
                tag_or_name: "ten".to_string(),
                address: addr_10,
                entrypoints: vec!["exec".into()],
            },
        );

        let policies = collect_policies_from_contracts(user_addr, &contracts).unwrap();

        // Controller canonical sort is done on normalized/padded address strings.
        // So 0x2 comes before 0x10.
        let first_two = policies
            .iter()
            .filter(|p| p.method == "exec")
            .take(2)
            .map(|p| p.target)
            .collect::<Vec<_>>();
        assert_eq!(first_two, vec![addr_2, addr_10]);
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

    #[test]
    fn parse_session_creation_response_decodes_full_encoded_payload() {
        let payload = serde_json::json!({
            "username": "alice",
            "address": "0x123",
            "ownerGuid": "0x456",
            "expiresAt": "1735689600",
            "transactionHash": "0x789",
            "alreadyRegistered": true,
            "allowedPoliciesRoot": "0x111",
            "metadataHash": "0x222",
            "sessionKeyGuid": "0x333",
            "guardianKeyGuid": "0x444"
        });
        let encoded = general_purpose::STANDARD_NO_PAD.encode(payload.to_string());

        let decoded = parse_session_creation_response(&encoded).unwrap();

        assert_eq!(decoded.username, "alice");
        assert_eq!(decoded.address, felt!("0x123"));
        assert_eq!(decoded.owner_guid, felt!("0x456"));
        assert_eq!(decoded.expires_at, "1735689600");
        assert_eq!(decoded.transaction_hash, Some(felt!("0x789")));
        assert!(decoded.already_registered);
        assert_eq!(decoded.allowed_policies_root, Some(felt!("0x111")));
        assert_eq!(decoded.metadata_hash, Some(felt!("0x222")));
        assert_eq!(decoded.session_key_guid, Some(felt!("0x333")));
        assert_eq!(decoded.guardian_key_guid, Some(felt!("0x444")));
    }

    #[test]
    fn alternate_policy_orders_produces_unique_deterministic_candidates() {
        let policies = vec![
            PolicyMethod { target: felt!("0x10"), method: "z".into() },
            PolicyMethod { target: felt!("0x2"), method: "a".into() },
            PolicyMethod { target: felt!("0x2"), method: "m".into() },
        ];

        let candidates = alternate_policy_orders(&policies);
        assert!(!candidates.is_empty());

        // No duplicates across candidate orderings.
        for i in 0..candidates.len() {
            for j in (i + 1)..candidates.len() {
                assert_ne!(candidates[i], candidates[j]);
            }
        }

        // At least one candidate should keep both addresses present (sanity).
        let has_both_addresses = candidates.iter().any(|candidate| {
            let targets = candidate.iter().map(|p| p.target).collect::<Vec<_>>();
            targets.contains(&felt!("0x2")) && targets.contains(&felt!("0x10"))
        });
        assert!(has_both_addresses);
    }

    #[test]
    fn alternate_policy_orders_keeps_method_order_variant() {
        let policies = vec![
            PolicyMethod { target: felt!("0x1"), method: "spawn".into() },
            PolicyMethod { target: felt!("0x2"), method: "move".into() },
            PolicyMethod { target: felt!("0x3"), method: "attack".into() },
        ];

        let candidates = alternate_policy_orders(&policies);
        let has_method_first = candidates.iter().any(|candidate| {
            candidate.iter().map(|p| p.method.as_str()).collect::<Vec<_>>()
                == vec!["attack", "move", "spawn"]
        });
        assert!(has_method_first);
    }

    #[test]
    fn is_equal_to_existing_ignores_order_and_authorized_toggle() {
        let new_policies = vec![
            PolicyMethod { target: felt!("0x1"), method: "spawn".into() },
            PolicyMethod { target: felt!("0x2"), method: "move".into() },
        ];

        let requested = vec![
            Policy::Call(CallPolicy {
                contract_address: felt!("0x2"),
                selector: get_selector_from_name("move").unwrap(),
                authorized: Some(false),
            }),
            Policy::Call(CallPolicy {
                contract_address: felt!("0x1"),
                selector: get_selector_from_name("spawn").unwrap(),
                authorized: Some(true),
            }),
        ];

        let session = session_with_requested_policies(requested);
        assert!(is_equal_to_existing(&new_policies, &session));
    }

    #[test]
    fn is_equal_to_existing_ignores_non_call_requested_policies() {
        let new_policies = vec![PolicyMethod { target: felt!("0x1"), method: "spawn".into() }];

        let requested = vec![
            Policy::Call(CallPolicy {
                contract_address: felt!("0x1"),
                selector: get_selector_from_name("spawn").unwrap(),
                authorized: Some(true),
            }),
            Policy::TypedData(TypedDataPolicy {
                scope_hash: felt!("0x123"),
                authorized: Some(true),
            }),
        ];

        let session = session_with_requested_policies(requested);
        assert!(is_equal_to_existing(&new_policies, &session));
    }

    #[test]
    fn is_equal_to_existing_detects_call_set_changes() {
        let new_policies = vec![
            PolicyMethod { target: felt!("0x1"), method: "spawn".into() },
            PolicyMethod { target: felt!("0x2"), method: "move".into() },
        ];

        let requested = vec![Policy::Call(CallPolicy {
            contract_address: felt!("0x1"),
            selector: get_selector_from_name("spawn").unwrap(),
            authorized: Some(true),
        })];

        let session = session_with_requested_policies(requested);
        assert!(!is_equal_to_existing(&new_policies, &session));
    }
}
