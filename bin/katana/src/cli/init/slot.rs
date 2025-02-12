use std::str::FromStr;

use anyhow::Result;
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
    #[arg(long)]
    pub slot: bool,

    #[arg(requires_all = ["id", "slot"])]
    #[arg(long = "slot.paymaster-accounts")]
    #[arg(value_parser = parse_paymaster_accounts_args)]
    pub paymaster_accounts: Option<Vec<PaymasterAccountArgs>>,
}

#[derive(Debug, Clone)]
pub struct PaymasterAccountArgs {
    /// The public key of the paymaster account.
    pub public_key: Felt,
}

fn parse_paymaster_accounts_args(value: &str) -> Result<Vec<PaymasterAccountArgs>> {
    let mut accounts = Vec::new();
    for s in value.split(',') {
        accounts.push(PaymasterAccountArgs { public_key: Felt::from_str(s)? });
    }
    Ok(accounts)
}

pub fn add_paymasters_to_genesis(
    genesis: &mut Genesis,
    slot_paymasters: &[PaymasterAccountArgs],
) -> Vec<ContractAddress> {
    let mut accounts = Vec::with_capacity(slot_paymasters.len());

    for paymaster in slot_paymasters {
        let public_key = paymaster.public_key;
        let class_hash = DEFAULT_ACCOUNT_CLASS_HASH;
        let balance = U256::from(DEFAULT_PREFUNDED_ACCOUNT_BALANCE);

        let (addr, account) = GenesisAccount::new_with_balance(public_key, class_hash, balance);
        let account = GenesisAllocation::Account(GenesisAccountAlloc::Account(account));
        accounts.push((addr, account));
    }

    let addresses: Vec<ContractAddress> = accounts.iter().map(|(addr, ..)| *addr).collect();
    genesis.extend_allocations(accounts);

    addresses
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_paymasters_to_genesis() {
        let mut genesis = Genesis::default();
        let mut paymasters = Vec::new();

        for i in 0..3 {
            paymasters.push(PaymasterAccountArgs { public_key: Felt::from(i) });
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
}
