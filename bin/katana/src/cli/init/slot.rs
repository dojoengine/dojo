use std::str::FromStr;

use anyhow::{anyhow, Result};
use clap::Args;
use katana_primitives::genesis::allocation::{
    GenesisAccount, GenesisAccountAlloc, GenesisAllocation,
};
use katana_primitives::genesis::constant::{
    DEFAULT_ACCOUNT_CLASS_HASH, DEFAULT_PREFUNDED_ACCOUNT_BALANCE,
};
use katana_primitives::genesis::Genesis;
use katana_primitives::{ContractAddress, Felt, U256};

#[derive(Debug, Args)]
#[command(next_help_heading = "Slot options")]
pub struct SlotArgs {
    /// Enable `slot`-specific features.
    #[arg(long)]
    pub slot: bool,

    /// Specify the number of paymaster accounts to create.
    ///
    /// This argument accepts a list of values, where each value is a pair of public key and salt
    /// separated by a comma.
    ///
    /// For example:
    ///
    /// ```
    /// --slot.paymasters 0x1,0x2 0x3,0x4 0x5,0x6
    /// ```
    ///
    /// where the total number of pairs determine how many paymaster accounts will be created.
    #[arg(requires_all = ["id", "slot"])]
    #[arg(long = "slot.paymasters", value_delimiter = ' ')]
    pub paymaster_accounts: Option<Vec<PaymasterAccountArgs>>,
}

#[derive(Debug, Clone)]
pub struct PaymasterAccountArgs {
    /// The public key of the paymaster account.
    pub public_key: Felt,
    /// The salt of the paymaster account.
    pub salt: Felt,
}

impl FromStr for PaymasterAccountArgs {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        let mut parts = s.split(',');

        let public_key = parts.next().ok_or_else(|| anyhow!("missing public key"))?;
        let salt = parts.next().ok_or_else(|| anyhow!("missing salt"))?;

        let public_key = Felt::from_str(public_key)?;
        let salt = Felt::from_str(salt)?;

        Ok(PaymasterAccountArgs { public_key, salt })
    }
}

pub fn add_paymasters_to_genesis(
    genesis: &mut Genesis,
    slot_paymasters: &[PaymasterAccountArgs],
) -> Vec<ContractAddress> {
    let mut accounts = Vec::with_capacity(slot_paymasters.len());

    for paymaster in slot_paymasters {
        let class_hash = DEFAULT_ACCOUNT_CLASS_HASH;
        let balance = U256::from(DEFAULT_PREFUNDED_ACCOUNT_BALANCE);

        let (addr, account) = GenesisAccount::new_with_salt_and_balance(
            paymaster.public_key,
            class_hash,
            paymaster.salt,
            balance,
        );

        let account = GenesisAllocation::Account(GenesisAccountAlloc::Account(account));
        accounts.push((addr, account));
    }

    let addresses: Vec<ContractAddress> = accounts.iter().map(|(addr, ..)| *addr).collect();
    genesis.extend_allocations(accounts);

    addresses
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;

    #[test]
    fn test_add_paymasters_to_genesis() {
        let mut genesis = Genesis::default();
        let mut paymasters = Vec::new();

        for i in 0..3 {
            paymasters
                .push(PaymasterAccountArgs { public_key: Felt::from(i), salt: Felt::from(i) });
        }

        let expected_addresses = add_paymasters_to_genesis(&mut genesis, &paymasters);

        for (i, addr) in expected_addresses.iter().enumerate() {
            let account = genesis.allocations.get(addr).expect("account missing");
            match account {
                GenesisAllocation::Account(GenesisAccountAlloc::Account(account)) => {
                    assert_eq!(account.public_key, Felt::from(i));
                    assert_eq!(account.class_hash, DEFAULT_ACCOUNT_CLASS_HASH);
                    assert_eq!(
                        account.balance,
                        Some(U256::from(DEFAULT_PREFUNDED_ACCOUNT_BALANCE))
                    );
                }

                _ => panic!("Expected GenesisAccountAlloc::Account"),
            }
        }
    }

    #[test]
    fn test_distinct_paymasters_same_pubkey() {
        let mut genesis = Genesis::default();
        let mut paymasters = Vec::new();
        let public_key = Felt::from(1);

        // Add multiple paymasters with same public key
        for i in 0..3 {
            let salt = Felt::from(i);
            paymasters.push(PaymasterAccountArgs { public_key, salt });
        }

        let addresses = add_paymasters_to_genesis(&mut genesis, &paymasters);

        // Verify addresses are unique
        let mut unique_addresses = addresses.clone();
        unique_addresses.sort();
        unique_addresses.dedup();

        assert_eq!(addresses.len(), unique_addresses.len(), "addresses are not unique");

        // Verify each paymaster has the same public key
        for addr in addresses {
            let account = genesis.allocations.get(&addr).expect("account missing");
            match account {
                GenesisAllocation::Account(GenesisAccountAlloc::Account(account)) => {
                    assert_eq!(account.public_key, public_key);
                }
                _ => panic!("Expected GenesisAccountAlloc::Account"),
            }
        }
    }

    #[test]
    fn test_parse_no_paymaster_args() {
        #[derive(Parser)]
        struct Cli {
            #[arg(long)]
            id: bool,
            #[command(flatten)]
            slot: SlotArgs,
        }

        let Cli { slot, .. } = Cli::parse_from(["cli", "--id", "--slot"]);
        assert!(slot.paymaster_accounts.is_none());
    }

    #[test]
    fn test_parse_paymaster_args() {
        #[derive(Parser)]
        struct Cli {
            #[arg(long)]
            id: bool,
            #[command(flatten)]
            slot: SlotArgs,
        }

        let Cli { slot, .. } = Cli::parse_from([
            "cli",
            "--id",
            "--slot",
            "--slot.paymasters",
            "0x1,0x2",
            "0x1,0x3",
            "0x1,0x4",
        ]);

        let paymasters = slot.paymaster_accounts.unwrap();
        assert_eq!(paymasters.len(), 3);

        assert_eq!(paymasters[0].public_key, Felt::from_str("0x1").unwrap());
        assert_eq!(paymasters[0].salt, Felt::from_str("0x2").unwrap());

        assert_eq!(paymasters[1].public_key, Felt::from_str("0x1").unwrap());
        assert_eq!(paymasters[1].salt, Felt::from_str("0x3").unwrap());

        assert_eq!(paymasters[2].public_key, Felt::from_str("0x1").unwrap());
        assert_eq!(paymasters[2].salt, Felt::from_str("0x4").unwrap());
    }
}
