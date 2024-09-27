use std::collections::HashMap;

use anyhow::Result;
use cainome::cairo_serde::{ByteArray, CairoSerde};
use starknet::core::types::{BlockId, BlockTag, Felt, FunctionCall, U256};
use starknet::core::utils::{get_selector_from_name, parse_cairo_short_string};
use starknet::providers::Provider;
use tracing::debug;

use super::query_queue::{Argument, QueryType};
use super::utils::{sql_string_to_u256, u256_to_sql_string, I256};
use super::{Sql, FELT_DELIMITER};
use crate::sql::utils::{felt_and_u256_to_sql_string, felt_to_sql_string, felts_to_sql_string};
use crate::utils::utc_dt_string_from_timestamp;

impl Sql {
    #[allow(clippy::too_many_arguments)]
    pub async fn handle_erc20_transfer<P: Provider + Sync>(
        &mut self,
        contract_address: Felt,
        from_address: Felt,
        to_address: Felt,
        amount: U256,
        provider: &P,
        block_timestamp: u64,
        cache: &mut HashMap<String, I256>,
    ) -> Result<()> {
        // contract_address
        let token_id = felt_to_sql_string(&contract_address);

        let token_exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM tokens WHERE id = ?)")
                .bind(token_id.clone())
                .fetch_one(&self.pool)
                .await?;

        if !token_exists {
            self.register_erc20_token_metadata(contract_address, &token_id, provider).await?;
        }

        self.store_erc_transfer_event(
            contract_address,
            from_address,
            to_address,
            amount,
            &token_id,
            block_timestamp,
        );

        self.query_queue.execute_all().await?;

        if from_address != Felt::ZERO {
            // from_address/contract_address/
            let from_balance_id = felts_to_sql_string(&[from_address, contract_address]);
            let from_balance = cache.entry(from_balance_id).or_default();
            *from_balance -= I256::from(amount);
        }

        if to_address != Felt::ZERO {
            let to_balance_id = felts_to_sql_string(&[to_address, contract_address]);
            let to_balance = cache.entry(to_balance_id).or_default();
            *to_balance += I256::from(amount);
        }

        if cache.len() >= 100000 {
            self.apply_cache_diff(cache).await?;
        }

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn handle_erc721_transfer<P: Provider + Sync>(
        &mut self,
        contract_address: Felt,
        from_address: Felt,
        to_address: Felt,
        token_id: U256,
        provider: &P,
        block_timestamp: u64,
        cache: &mut HashMap<String, I256>,
    ) -> Result<()> {
        // contract_address:id
        let token_id = felt_and_u256_to_sql_string(&contract_address, &token_id);
        let token_exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM tokens WHERE id = ?)")
                .bind(token_id.clone())
                .fetch_one(&self.pool)
                .await?;

        if !token_exists {
            self.register_erc721_token_metadata(contract_address, &token_id, provider).await?;
        }

        self.store_erc_transfer_event(
            contract_address,
            from_address,
            to_address,
            U256::from(1u8),
            &token_id,
            block_timestamp,
        );

        self.query_queue.execute_all().await?;

        // from_address/contract_address:id
        if from_address != Felt::ZERO {
            let from_balance_id =
                format!("{}{FELT_DELIMITER}{}", felt_to_sql_string(&from_address), &token_id);
            let from_balance = cache.entry(from_balance_id).or_default();
            *from_balance -= I256::from(1u8);
        }

        if to_address != Felt::ZERO {
            let to_balance_id =
                format!("{}{FELT_DELIMITER}{}", felt_to_sql_string(&to_address), &token_id);
            let to_balance = cache.entry(to_balance_id).or_default();
            *to_balance += I256::from(1u8);
        }

        if cache.len() >= 100000 {
            self.apply_cache_diff(cache).await?;
        }

        Ok(())
    }

    async fn register_erc20_token_metadata<P: Provider + Sync>(
        &mut self,
        contract_address: Felt,
        token_id: &str,
        provider: &P,
    ) -> Result<()> {
        // Fetch token information from the chain
        let name = provider
            .call(
                FunctionCall {
                    contract_address,
                    entry_point_selector: get_selector_from_name("name").unwrap(),
                    calldata: vec![],
                },
                BlockId::Tag(BlockTag::Pending),
            )
            .await?;

        // len = 1 => return value felt (i.e. legacy erc20 token)
        // len > 1 => return value ByteArray (i.e. new erc20 token)
        let name = if name.len() == 1 {
            parse_cairo_short_string(&name[0]).unwrap()
        } else {
            ByteArray::cairo_deserialize(&name, 0)
                .expect("Return value not ByteArray")
                .to_string()
                .expect("Return value not String")
        };

        let symbol = provider
            .call(
                FunctionCall {
                    contract_address,
                    entry_point_selector: get_selector_from_name("symbol").unwrap(),
                    calldata: vec![],
                },
                BlockId::Tag(BlockTag::Pending),
            )
            .await?;

        let symbol = if symbol.len() == 1 {
            parse_cairo_short_string(&symbol[0]).unwrap()
        } else {
            ByteArray::cairo_deserialize(&symbol, 0)
                .expect("Return value not ByteArray")
                .to_string()
                .expect("Return value not String")
        };

        let decimals = provider
            .call(
                FunctionCall {
                    contract_address,
                    entry_point_selector: get_selector_from_name("decimals").unwrap(),
                    calldata: vec![],
                },
                BlockId::Tag(BlockTag::Pending),
            )
            .await?;
        let decimals = u8::cairo_deserialize(&decimals, 0).expect("Return value not u8");

        // Insert the token into the tokens table
        self.query_queue.enqueue(
            "INSERT INTO tokens (id, contract_address, name, symbol, decimals) VALUES (?, ?, ?, \
             ?, ?)",
            vec![
                Argument::String(token_id.to_string()),
                Argument::FieldElement(contract_address),
                Argument::String(name),
                Argument::String(symbol),
                Argument::Int(decimals.into()),
            ],
            QueryType::Other,
        );

        Ok(())
    }

    async fn register_erc721_token_metadata<P: Provider + Sync>(
        &mut self,
        contract_address: Felt,
        token_id: &str,
        provider: &P,
    ) -> Result<()> {
        let res = sqlx::query_as::<_, (String, String, u8)>(
            "SELECT name, symbol, decimals FROM tokens WHERE contract_address = ?",
        )
        .bind(felt_to_sql_string(&contract_address))
        .fetch_one(&self.pool)
        .await;

        // If we find a token already registered for this contract_address we dont need to refetch
        // the data since its same for all ERC721 tokens
        if let Ok((name, symbol, decimals)) = res {
            debug!(
                contract_address = %felt_to_sql_string(&contract_address),
                "Token already registered for contract_address, so reusing fetched data",
            );
            self.query_queue.enqueue(
                "INSERT INTO tokens (id, contract_address, name, symbol, decimals) VALUES (?, ?, \
                 ?, ?, ?)",
                vec![
                    Argument::String(token_id.to_string()),
                    Argument::FieldElement(contract_address),
                    Argument::String(name),
                    Argument::String(symbol),
                    Argument::Int(decimals.into()),
                ],
                QueryType::Other,
            );
            return Ok(());
        }

        // Fetch token information from the chain
        let name = provider
            .call(
                FunctionCall {
                    contract_address,
                    entry_point_selector: get_selector_from_name("name").unwrap(),
                    calldata: vec![],
                },
                BlockId::Tag(BlockTag::Pending),
            )
            .await?;

        // len = 1 => return value felt (i.e. legacy erc721 token)
        // len > 1 => return value ByteArray (i.e. new erc721 token)
        let name = if name.len() == 1 {
            parse_cairo_short_string(&name[0]).unwrap()
        } else {
            ByteArray::cairo_deserialize(&name, 0)
                .expect("Return value not ByteArray")
                .to_string()
                .expect("Return value not String")
        };

        let symbol = provider
            .call(
                FunctionCall {
                    contract_address,
                    entry_point_selector: get_selector_from_name("symbol").unwrap(),
                    calldata: vec![],
                },
                BlockId::Tag(BlockTag::Pending),
            )
            .await?;
        let symbol = if symbol.len() == 1 {
            parse_cairo_short_string(&symbol[0]).unwrap()
        } else {
            ByteArray::cairo_deserialize(&symbol, 0)
                .expect("Return value not ByteArray")
                .to_string()
                .expect("Return value not String")
        };

        let decimals = 0;

        // Insert the token into the tokens table
        self.query_queue.enqueue(
            "INSERT INTO tokens (id, contract_address, name, symbol, decimals) VALUES (?, ?, ?, \
             ?, ?)",
            vec![
                Argument::String(token_id.to_string()),
                Argument::FieldElement(contract_address),
                Argument::String(name),
                Argument::String(symbol),
                Argument::Int(decimals.into()),
            ],
            QueryType::Other,
        );

        Ok(())
    }

    fn store_erc_transfer_event(
        &mut self,
        contract_address: Felt,
        from: Felt,
        to: Felt,
        amount: U256,
        token_id: &str,
        block_timestamp: u64,
    ) {
        let insert_query = "INSERT INTO erc_transfers (contract_address, from_address, \
                            to_address, amount, token_id, executed_at) VALUES (?, ?, ?, ?, ?, ?)";

        self.query_queue.enqueue(
            insert_query,
            vec![
                Argument::FieldElement(contract_address),
                Argument::FieldElement(from),
                Argument::FieldElement(to),
                Argument::String(u256_to_sql_string(&amount)),
                Argument::String(token_id.to_string()),
                Argument::String(utc_dt_string_from_timestamp(block_timestamp)),
            ],
            QueryType::Other,
        );
    }

    pub async fn apply_cache_diff(&mut self, cache: &mut HashMap<String, I256>) -> Result<()> {
        for (id_str, balance) in cache.iter() {
            let id = id_str.split(FELT_DELIMITER).collect::<Vec<&str>>();
            match id.len() {
                // account_address/contract_address:id => ERC721
                2 => {
                    let account_address = id[0];
                    let token_id = id[1];
                    let mid = token_id.split(":").collect::<Vec<&str>>();
                    let contract_address = mid[0];

                    self.apply_balance_diff(
                        id_str,
                        account_address,
                        contract_address,
                        token_id,
                        balance,
                    )
                    .await?;
                }
                // account_address/contract_address/ => ERC20
                3 => {
                    let account_address = id[0];
                    let contract_address = id[1];
                    let token_id = id[1];

                    self.apply_balance_diff(
                        id_str,
                        account_address,
                        contract_address,
                        token_id,
                        balance,
                    )
                    .await?;
                }
                _ => unreachable!(),
            }
        }

        cache.clear();
        Ok(())
    }

    async fn apply_balance_diff(
        &self,
        id: &str,
        account_address: &str,
        contract_address: &str,
        token_id: &str,
        balance_diff: &I256,
    ) -> Result<()> {
        let balance: Option<(String,)> =
            sqlx::query_as("SELECT balance FROM balances WHERE id = ? ")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?;

        let mut balance = if let Some(balance) = balance {
            sql_string_to_u256(&balance.0)
        } else {
            U256::from(0u8)
        };

        if balance_diff.is_negative {
            balance -= balance_diff.value;
        } else {
            balance += balance_diff.value;
        }

        // write the new balance to the database
        sqlx::query(
            "INSERT OR REPLACE INTO balances (id, contract_address, account_address, token_id, \
             balance) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(id)
        .bind(contract_address)
        .bind(account_address)
        .bind(token_id)
        .bind(u256_to_sql_string(&balance))
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}
