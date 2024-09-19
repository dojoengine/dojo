use std::ops::{Add, Sub};

use anyhow::Result;
use cainome::cairo_serde::{ByteArray, CairoSerde};
use starknet::core::types::{BlockId, BlockTag, Felt, FunctionCall, U256};
use starknet::core::utils::{get_selector_from_name, parse_cairo_short_string};
use starknet::providers::Provider;
use tracing::debug;

use super::query_queue::{Argument, QueryType};
use super::utils::{sql_string_to_u256, u256_to_sql_string};
use super::{Sql, FELT_DELIMITER};
use crate::sql::utils::{felt_and_u256_to_sql_string, felt_to_sql_string, felts_to_sql_string};
use crate::utils::utc_dt_string_from_timestamp;

impl Sql {
    pub async fn handle_erc20_transfer<P: Provider + Sync>(
        &mut self,
        contract_address: Felt,
        from_address: Felt,
        to_address: Felt,
        amount: U256,
        provider: &P,
        block_timestamp: u64,
    ) -> Result<()> {
        // unique token identifier in DB
        let token_id = felt_to_sql_string(&contract_address);

        let token_exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM tokens WHERE id = ?)")
                .bind(token_id.clone())
                .fetch_one(&self.pool)
                .await?;

        if !token_exists {
            self.register_erc20_token_metadata(contract_address, &token_id, provider).await?;
        }

        self.register_erc_transfer_event(
            contract_address,
            from_address,
            to_address,
            amount,
            &token_id,
            block_timestamp,
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
            .bind(felt_to_sql_string(&contract_address))
            .bind(felt_to_sql_string(&from_address))
            .bind(felt_to_sql_string(&to_address));

            // (address, balance)
            let balances: Vec<(String, String)> = query.fetch_all(&self.pool).await?;
            // (address, balance) is primary key in DB, and we are fetching for 2 addresses so there
            // should be at most 2 rows returned
            assert!(balances.len() <= 2);

            let from_balance = balances
                .iter()
                .find(|(address, _)| address == &felt_to_sql_string(&from_address))
                .map(|(_, balance)| balance.clone())
                .unwrap_or_else(|| u256_to_sql_string(&U256::from(0u8)));

            let to_balance = balances
                .iter()
                .find(|(address, _)| address == &felt_to_sql_string(&to_address))
                .map(|(_, balance)| balance.clone())
                .unwrap_or_else(|| u256_to_sql_string(&U256::from(0u8)));

            let from_balance = sql_string_to_u256(&from_balance);
            let to_balance = sql_string_to_u256(&to_balance);

            let new_from_balance =
                if from_address != Felt::ZERO { from_balance.sub(amount) } else { from_balance };
            let new_to_balance =
                if to_address != Felt::ZERO { to_balance.add(amount) } else { to_balance };

            let update_query = "
            INSERT INTO balances (id, balance, account_address, contract_address, token_id)
            VALUES (?, ?, ?, ?, ?)
            ON CONFLICT (id) 
            DO UPDATE SET balance = excluded.balance";

            if from_address != Felt::ZERO {
                self.query_queue.enqueue(
                    update_query,
                    vec![
                        Argument::String(felts_to_sql_string(&[from_address, contract_address])),
                        Argument::String(u256_to_sql_string(&new_from_balance)),
                        Argument::FieldElement(from_address),
                        Argument::FieldElement(contract_address),
                        Argument::String(token_id.clone()),
                    ],
                    QueryType::Other,
                );
            }

            if to_address != Felt::ZERO {
                self.query_queue.enqueue(
                    update_query,
                    vec![
                        Argument::String(felts_to_sql_string(&[to_address, contract_address])),
                        Argument::String(u256_to_sql_string(&new_to_balance)),
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

    pub async fn handle_erc721_transfer<P: Provider + Sync>(
        &mut self,
        contract_address: Felt,
        from_address: Felt,
        to_address: Felt,
        token_id: U256,
        provider: &P,
        block_timestamp: u64,
    ) -> Result<()> {
        let token_id = felt_and_u256_to_sql_string(&contract_address, &token_id);
        let token_exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM tokens WHERE id = ?)")
                .bind(token_id.clone())
                .fetch_one(&self.pool)
                .await?;

        if !token_exists {
            self.register_erc721_token_metadata(contract_address, &token_id, provider).await?;
        }

        self.register_erc_transfer_event(
            contract_address,
            from_address,
            to_address,
            U256::from(1u8),
            &token_id,
            block_timestamp,
        );

        // Update balances in erc721_balances table
        {
            let update_query = "
            INSERT INTO balances (id, balance, account_address, contract_address, token_id)
            VALUES (?, ?, ?, ?, ?)
            ON CONFLICT (id) 
            DO UPDATE SET balance = excluded.balance";

            if from_address != Felt::ZERO {
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

            if to_address != Felt::ZERO {
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

    fn register_erc_transfer_event(
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
}
