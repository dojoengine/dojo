use anyhow::Result;
use cainome::cairo_serde::ByteArray;
use cainome::cairo_serde::CairoSerde;
use starknet::core::types::{BlockId, BlockTag, FunctionCall, U256};
use starknet::core::utils::{get_selector_from_name, parse_cairo_short_string};
use starknet::{core::types::Felt, providers::Provider};
use std::ops::{Add, Sub};

use super::query_queue::{Argument, QueryQueue, QueryType};
use super::utils::{sql_string_to_u256, u256_to_sql_string};
use crate::utils::utc_dt_string_from_timestamp;

use super::Sql;

impl Sql {
    pub async fn handle_erc20_transfer<P: Provider + Sync>(
        &mut self,
        contract_address: Felt,
        from: Felt,
        to: Felt,
        amount: U256,
        provider: &P,
        block_timestamp: u64,
    ) -> Result<()> {
        // unique token identifier in DB
        let token_id = format!("{:#x}", contract_address);

        let token_exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM tokens WHERE id = ?)")
                .bind(token_id.clone())
                .fetch_one(&self.pool)
                .await?;

        if !token_exists {
            register_erc20_token_metadata(
                contract_address,
                &mut self.query_queue,
                &token_id,
                provider,
            )
            .await?;
        }

        register_erc_transfer_event(
            contract_address,
            from,
            to,
            amount,
            &token_id,
            block_timestamp,
            &mut self.query_queue,
        );

        // Update balances in erc20_balance table
        {
            // NOTE: formatting here should match the format we use for Argument type in QueryQueue
            // TODO: abstract this so they cannot mismatch

            // Since balance are stored as TEXT in db, we cannot directly use INSERT OR UPDATE
            // statements.
            // Fetch balances for both `from` and `to` addresses, update them and write back to db
            let query = sqlx::query_as::<_, (String, String)>(
                "SELECT account_address, balance FROM balances WHERE contract_address = ? AND \
                 account_address IN (?, ?)",
            )
            .bind(format!("{:#x}", contract_address))
            .bind(format!("{:#x}", from))
            .bind(format!("{:#x}", to));

            // (address, balance)
            let balances: Vec<(String, String)> = query.fetch_all(&self.pool).await?;
            // (address, balance) is primary key in DB, and we are fetching for 2 addresses so there
            // should be at most 2 rows returned
            assert!(balances.len() <= 2);

            let from_balance = balances
                .iter()
                .find(|(address, _)| address == &format!("{:#x}", from))
                .map(|(_, balance)| balance.clone())
                .unwrap_or_else(|| format!("{:#64x}", crypto_bigint::U256::ZERO));

            let to_balance = balances
                .iter()
                .find(|(address, _)| address == &format!("{:#x}", to))
                .map(|(_, balance)| balance.clone())
                .unwrap_or_else(|| format!("{:#64x}", crypto_bigint::U256::ZERO));

            let from_balance = sql_string_to_u256(&from_balance);
            let to_balance = sql_string_to_u256(&to_balance);

            let new_from_balance =
                if from != Felt::ZERO { from_balance.sub(amount) } else { from_balance };
            let new_to_balance = if to != Felt::ZERO { to_balance.add(amount) } else { to_balance };

            let update_query = "
            INSERT INTO balances (id, balance, account_address, contract_address, token_id)
            VALUES (?, ?, ?, ?, ?)
            ON CONFLICT (id) 
            DO UPDATE SET balance = excluded.balance";

            if from != Felt::ZERO {
                self.query_queue.enqueue(
                    update_query,
                    vec![
                        Argument::String(format!("{:#x}:{:#x}", from, contract_address)),
                        Argument::String(u256_to_sql_string(&new_from_balance)),
                        Argument::FieldElement(from),
                        Argument::FieldElement(contract_address),
                        Argument::String(token_id.clone()),
                    ],
                    QueryType::Other,
                );
            }

            if to != Felt::ZERO {
                self.query_queue.enqueue(
                    update_query,
                    vec![
                        Argument::String(format!("{:#x}:{:#x}", to, contract_address)),
                        Argument::String(u256_to_sql_string(&new_to_balance)),
                        Argument::FieldElement(to),
                        Argument::FieldElement(contract_address),
                        Argument::String(token_id.clone()),
                    ],
                    QueryType::Other,
                );
            }
        }
        self.query_queue.execute_all().await?;

        Ok(())
    }

    pub async fn handle_erc721_transfer<P: Provider + Sync>(
        &mut self,
        contract_address: Felt,
        from: Felt,
        to: Felt,
        token_id: U256,
        provider: &P,
        block_timestamp: u64,
    ) -> Result<()> {
        let token_id = format!("{:#x}:{}", contract_address, u256_to_sql_string(&token_id));
        let token_exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM tokens WHERE id = ?)")
                .bind(token_id.clone())
                .fetch_one(&self.pool)
                .await?;

        if !token_exists {
            register_erc721_token_metadata(
                contract_address,
                &mut self.query_queue,
                &token_id,
                provider,
            )
            .await?;
        }

        register_erc_transfer_event(
            contract_address,
            from,
            to,
            U256::from(1u8),
            &token_id,
            block_timestamp,
            &mut self.query_queue,
        );

        // Update balances in erc721_balances table
        {
            let update_query = "
            INSERT INTO balances (id, balance, account_address, contract_address, token_id)
            VALUES (?, ?, ?, ?, ?)
            ON CONFLICT (id) 
            DO UPDATE SET balance = excluded.balance";

            if from != Felt::ZERO {
                self.query_queue.enqueue(
                    update_query,
                    vec![
                        Argument::String(format!(
                            "{}{FELT_DELIMITER}{}",
                            felt_to_sql_string(&from_address),
                            &token_id
                        )),
                        Argument::String(u256_to_sql_string(&U256::from(0u8))),
                        Argument::FieldElement(from_address),
                        Argument::FieldElement(contract_address),
                        Argument::String(token_id.clone()),
                    ],
                    QueryType::Other,
                );
            }

            if to != Felt::ZERO {
                self.query_queue.enqueue(
                    update_query,
                    vec![
                        Argument::String(format!(
                            "{}{FELT_DELIMITER}{}",
                            felt_to_sql_string(&to_address),
                            &token_id
                        )),
                        Argument::String(u256_to_sql_string(&U256::from(1u8))),
                        Argument::FieldElement(to_address),
                        Argument::FieldElement(contract_address),
                        Argument::String(token_id.clone()),
                    ],
                    QueryType::Other,
                );
            }
        }
        self.query_queue.execute_all().await?;

        Ok(())
    }
}

async fn register_erc20_token_metadata<P: Provider + Sync>(
    contract_address: Felt,
    queue: &mut QueryQueue,
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
    queue.enqueue(
        "INSERT INTO tokens (id, contract_address, name, symbol, decimals) VALUES (?, ?, ?, ?, ?)",
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
    contract_address: Felt,
    queue: &mut QueryQueue,
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
    queue.enqueue(
        "INSERT INTO tokens (id, contract_address, name, symbol, decimals) VALUES (?, ?, ?, ?, ?)",
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

fn register_erc_transfer_event(
    contract_address: Felt,
    from: Felt,
    to: Felt,
    amount: U256,
    token_id: &str,
    block_timestamp: u64,
    queue: &mut QueryQueue,
) {
    let insert_query = "INSERT INTO erc_transfers (contract_address, from_address, to_address, \
                        amount, token_id, executed_at) VALUES (?, ?, ?, ?, ?, ?)";

    queue.enqueue(
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
