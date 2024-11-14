use std::str::FromStr;
use std::sync::Arc;

use anyhow::{Context, Result};
use cainome::cairo_serde::{ByteArray, CairoSerde};
use data_url::mime::Mime;
use data_url::DataUrl;
use reqwest::Client;
use starknet::core::types::{BlockId, BlockTag, FunctionCall, U256};
use starknet::core::utils::{get_selector_from_name, parse_cairo_short_string};
use starknet::providers::Provider;
use starknet_crypto::Felt;
use tracing::{debug, trace};

use super::{ApplyBalanceDiffQuery, Executor};
use crate::constants::TOKEN_BALANCE_TABLE;
use crate::sql::utils::{felt_to_sql_string, sql_string_to_u256, u256_to_sql_string, I256};
use crate::sql::FELT_DELIMITER;
use crate::types::ContractType;
use crate::utils::{fetch_content_from_ipfs, MAX_RETRY};

#[derive(Debug, Clone)]
pub struct RegisterErc721TokenQuery {
    pub token_id: String,
    pub contract_address: Felt,
    pub actual_token_id: U256,
}

#[derive(Debug, Clone)]
pub struct RegisterErc721TokenMetadata {
    pub query: RegisterErc721TokenQuery,
    pub name: String,
    pub symbol: String,
    pub metadata: String,
}

#[derive(Debug, Clone)]
pub struct RegisterErc20TokenQuery {
    pub token_id: String,
    pub contract_address: Felt,
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
}

impl<'c, P: Provider + Sync + Send + 'static> Executor<'c, P> {
    pub async fn apply_balance_diff(
        &mut self,
        apply_balance_diff: ApplyBalanceDiffQuery,
    ) -> Result<()> {
        let erc_cache = apply_balance_diff.erc_cache;
        for ((contract_type, id_str), balance) in erc_cache.iter() {
            let id = id_str.split(FELT_DELIMITER).collect::<Vec<&str>>();
            match contract_type {
                ContractType::WORLD => unreachable!(),
                ContractType::ERC721 => {
                    // account_address/contract_address:id => ERC721
                    assert!(id.len() == 2);
                    let account_address = id[0];
                    let token_id = id[1];
                    let mid = token_id.split(":").collect::<Vec<&str>>();
                    let contract_address = mid[0];

                    self.apply_balance_diff_helper(
                        id_str,
                        account_address,
                        contract_address,
                        token_id,
                        balance,
                    )
                    .await
                    .with_context(|| "Failed to apply balance diff in apply_cache_diff")?;
                }
                ContractType::ERC20 => {
                    // account_address/contract_address/ => ERC20
                    assert!(id.len() == 3);
                    let account_address = id[0];
                    let contract_address = id[1];
                    let token_id = id[1];

                    self.apply_balance_diff_helper(
                        id_str,
                        account_address,
                        contract_address,
                        token_id,
                        balance,
                    )
                    .await
                    .with_context(|| "Failed to apply balance diff in apply_cache_diff")?;
                }
            }
        }

        Ok(())
    }

    pub async fn apply_balance_diff_helper(
        &mut self,
        id: &str,
        account_address: &str,
        contract_address: &str,
        token_id: &str,
        balance_diff: &I256,
    ) -> Result<()> {
        let tx = &mut self.transaction;
        let balance: Option<(String,)> =
            sqlx::query_as(&format!("SELECT balance FROM {TOKEN_BALANCE_TABLE} WHERE id = ?"))
                .bind(id)
                .fetch_optional(&mut **tx)
                .await?;

        let mut balance = if let Some(balance) = balance {
            sql_string_to_u256(&balance.0)
        } else {
            U256::from(0u8)
        };

        if balance_diff.is_negative {
            if balance < balance_diff.value {
                dbg!(&balance_diff, balance, id);
            }
            balance -= balance_diff.value;
        } else {
            balance += balance_diff.value;
        }

        // write the new balance to the database
        sqlx::query(&format!(
            "INSERT OR REPLACE INTO {TOKEN_BALANCE_TABLE} (id, contract_address, account_address, \
             token_id, balance) VALUES (?, ?, ?, ?, ?)",
        ))
        .bind(id)
        .bind(contract_address)
        .bind(account_address)
        .bind(token_id)
        .bind(u256_to_sql_string(&balance))
        .execute(&mut **tx)
        .await?;

        Ok(())
    }

    pub async fn process_register_erc721_token_query(
        register_erc721_token: RegisterErc721TokenQuery,
        provider: Arc<P>,
        name: String,
        symbol: String,
    ) -> Result<RegisterErc721TokenMetadata> {
        let token_uri = if let Ok(token_uri) = provider
            .call(
                FunctionCall {
                    contract_address: register_erc721_token.contract_address,
                    entry_point_selector: get_selector_from_name("token_uri").unwrap(),
                    calldata: vec![
                        register_erc721_token.actual_token_id.low().into(),
                        register_erc721_token.actual_token_id.high().into(),
                    ],
                },
                BlockId::Tag(BlockTag::Pending),
            )
            .await
        {
            token_uri
        } else if let Ok(token_uri) = provider
            .call(
                FunctionCall {
                    contract_address: register_erc721_token.contract_address,
                    entry_point_selector: get_selector_from_name("tokenURI").unwrap(),
                    calldata: vec![
                        register_erc721_token.actual_token_id.low().into(),
                        register_erc721_token.actual_token_id.high().into(),
                    ],
                },
                BlockId::Tag(BlockTag::Pending),
            )
            .await
        {
            token_uri
        } else {
            return Err(anyhow::anyhow!("Failed to fetch token_uri"));
        };

        let token_uri = if let Ok(byte_array) = ByteArray::cairo_deserialize(&token_uri, 0) {
            byte_array.to_string().expect("Return value not String")
        } else if let Ok(felt_array) = Vec::<Felt>::cairo_deserialize(&token_uri, 0) {
            felt_array
                .iter()
                .map(parse_cairo_short_string)
                .collect::<Result<Vec<String>, _>>()
                .map(|strings| strings.join(""))
                .map_err(|_| anyhow::anyhow!("Failed parsing Array<Felt> to String"))?
        } else {
            return Err(anyhow::anyhow!("token_uri is neither ByteArray nor Array<Felt>"));
        };

        let metadata = Self::fetch_metadata(&token_uri).await.with_context(|| {
            format!(
                "Failed to fetch metadata for token_id: {}",
                register_erc721_token.actual_token_id
            )
        })?;
        let metadata = serde_json::to_string(&metadata).context("Failed to serialize metadata")?;
        Ok(RegisterErc721TokenMetadata { query: register_erc721_token, metadata, name, symbol })
    }

    // given a uri which can be either http/https url or data uri, fetch the metadata erc721
    // metadata json schema
    pub async fn fetch_metadata(token_uri: &str) -> Result<serde_json::Value> {
        // Parse the token_uri

        match token_uri {
            uri if uri.starts_with("http") || uri.starts_with("https") => {
                // Fetch metadata from HTTP/HTTPS URL
                debug!(token_uri = %token_uri, "Fetching metadata from http/https URL");
                let client = Client::new();
                let response = client
                    .get(token_uri)
                    .send()
                    .await
                    .context("Failed to fetch metadata from URL")?;

                let bytes = response.bytes().await.context("Failed to read response bytes")?;
                let json: serde_json::Value = serde_json::from_slice(&bytes)
                    .context(format!("Failed to parse metadata JSON from response: {:?}", bytes))?;

                Ok(json)
            }
            uri if uri.starts_with("ipfs") => {
                let cid = uri.strip_prefix("ipfs://").unwrap();
                debug!(cid = %cid, "Fetching metadata from IPFS");
                let response = fetch_content_from_ipfs(cid, MAX_RETRY)
                    .await
                    .context("Failed to fetch metadata from IPFS")?;

                let json: serde_json::Value =
                    serde_json::from_slice(&response).context(format!(
                        "Failed to parse metadata JSON from IPFS: {:?}, data: {:?}",
                        cid, &response
                    ))?;

                Ok(json)
            }
            uri if uri.starts_with("data") => {
                // Parse and decode data URI
                debug!("Parsing metadata from data URI");
                trace!(data_uri = %token_uri);

                // HACK: https://github.com/servo/rust-url/issues/908
                let uri = token_uri.replace("#", "%23");

                let data_url = DataUrl::process(&uri).context("Failed to parse data URI")?;

                // Ensure the MIME type is JSON
                if data_url.mime_type() != &Mime::from_str("application/json").unwrap() {
                    return Err(anyhow::anyhow!("Data URI is not of JSON type"));
                }

                let decoded = data_url.decode_to_vec().context("Failed to decode data URI")?;
                // HACK: Loot Survior NFT metadata contains control characters which makes the json
                // DATA invalid so filter them out
                let decoded_str = String::from_utf8_lossy(&decoded.0)
                    .chars()
                    .filter(|c| !c.is_ascii_control())
                    .collect::<String>();

                let json: serde_json::Value = serde_json::from_str(&decoded_str)
                    .context(format!("Failed to parse metadata JSON from data URI: {}", &uri))?;

                Ok(json)
            }
            uri => Err(anyhow::anyhow!("Unsupported URI scheme found in token URI: {}", uri)),
        }
    }

    pub async fn handle_erc721_token_metadata(
        &mut self,
        result: RegisterErc721TokenMetadata,
    ) -> Result<()> {
        let query = sqlx::query(
            "INSERT INTO tokens (id, contract_address, name, symbol, decimals, metadata) VALUES \
             (?, ?, ?, ?, ?, ?)",
        )
        .bind(&result.query.token_id)
        .bind(felt_to_sql_string(&result.query.contract_address))
        .bind(&result.name)
        .bind(&result.symbol)
        .bind(0)
        .bind(&result.metadata);

        query
            .execute(&mut *self.transaction)
            .await
            .with_context(|| format!("Failed to execute721Token query: {:?}", result))?;

        Ok(())
    }
}
