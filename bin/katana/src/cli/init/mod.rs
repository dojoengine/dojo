mod deployment;

use std::str::FromStr;
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::Args;
use inquire::{Confirm, CustomType, Select};
use katana_chain_spec::{FeeContracts, SettlementLayer, DEV_UNALLOCATED};
use katana_primitives::chain::ChainId;
use katana_primitives::genesis::allocation::DevAllocationsGenerator;
use katana_primitives::genesis::Genesis;
use katana_primitives::{ContractAddress, Felt};
use lazy_static::lazy_static;
use starknet::accounts::{ExecutionEncoding, SingleOwnerAccount};
use starknet::core::types::{BlockId, BlockTag};
use starknet::core::utils::{cairo_short_string_to_felt, parse_cairo_short_string};
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::{JsonRpcClient, Provider, Url};
use starknet::signers::{LocalWallet, SigningKey};
use tokio::runtime::Runtime;

const CARTRIDGE_SN_SEPOLIA_PROVIDER: &str = "https://api.cartridge.gg/x/starknet/sepolia";

#[derive(Debug, Args)]
pub struct InitArgs;

impl InitArgs {
    // TODO:
    // - deploy bridge contract
    // - generate the genesis
    pub(crate) fn execute(self) -> Result<()> {
        let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build()?;
        let input = self.prompt(&rt)?;

        let settlement = SettlementLayer::Starknet {
            account: input.account,
            rpc_url: input.rpc_url,
            id: ChainId::parse(&input.settlement_id)?,
            core_contract: input.settlement_contract,
        };

        let id = ChainId::parse(&input.id)?;
        let genesis = GENESIS.clone();
        let settlement = Some(settlement);
        let fee_contracts = FeeContracts {
            eth: DEFAULT_APPCHAIN_FEE_TOKEN_ADDRESS.into(),
            strk: DEFAULT_APPCHAIN_FEE_TOKEN_ADDRESS.into(),
        };

        let chain_spec = ChainSpec { id, genesis, settlement, fee_contracts };

        katana_chain_spec::file::write(&chain_spec).context("failed to write chain spec file")?;

        Ok(())
    }

    fn prompt(&self, rt: &Runtime) -> Result<PromptOutcome> {
        let chain_id = CustomType::<String>::new("Id")
        .with_help_message("This will be the id of your rollup chain.")
        // checks that the input is a valid ascii string.
        .with_parser(&|input| {
            if input.is_ascii() {
                Ok(input.to_string())
            } else {
                Err(())
            }
        })
        .with_error_message("Must be valid ASCII characters")
        .prompt()?;

        #[derive(Debug, strum_macros::Display)]
        enum SettlementChainOpt {
            Sepolia,
            #[cfg(feature = "init-custom-settlement-chain")]
            Custom,
        }

        // Right now we only support settling on Starknet Sepolia because we're limited to what
        // network the Atlantic service could settle the proofs to. Supporting a custom
        // network here (eg local devnet) would require that the proving service we're using
        // be able to settle the proofs there.
        let network_opts = vec![
            SettlementChainOpt::Sepolia,
            #[cfg(feature = "init-custom-settlement-chain")]
            SettlementChainOpt::Custom,
        ];

        let network_type = Select::new("Settlement chain", network_opts).prompt()?;

        let settlement_url = match network_type {
            SettlementChainOpt::Sepolia => Url::parse(CARTRIDGE_SN_SEPOLIA_PROVIDER)?,

            // Useful for testing the program flow without having to run it against actual network.
            #[cfg(feature = "init-custom-settlement-chain")]
            SettlementChainOpt::Custom => CustomType::<Url>::new("Settlement RPC URL")
                .with_default(Url::parse("http://localhost:5050")?)
                .with_error_message("Please enter a valid URL")
                .prompt()?,
        };

        let l1_provider = Arc::new(JsonRpcClient::new(HttpTransport::new(settlement_url.clone())));

        let contract_exist_parser = &|input: &str| {
            let block_id = BlockId::Tag(BlockTag::Pending);
            let address = Felt::from_str(input).map_err(|_| ())?;
            let result = rt.block_on(l1_provider.clone().get_class_hash_at(block_id, address));

            match result {
                Ok(..) => Ok(ContractAddress::from(address)),
                Err(..) => Err(()),
            }
        };

        let account_address = CustomType::<ContractAddress>::new("Account")
            .with_error_message("Please enter a valid account address")
            .with_parser(contract_exist_parser)
            .prompt()?;

        let private_key = CustomType::<Felt>::new("Private key")
            .with_formatter(&|input: Felt| format!("{input:#x}"))
            .prompt()?;

        let l1_chain_id = rt.block_on(l1_provider.chain_id())?;
        let account = SingleOwnerAccount::new(
            l1_provider.clone(),
            LocalWallet::from_signing_key(SigningKey::from_secret_scalar(private_key)),
            account_address.into(),
            l1_chain_id,
            ExecutionEncoding::New,
        );

        // TODO: uncomment once we actually using the fee token.
        // // The L1 fee token. Must be an existing token.
        // let fee_token = CustomType::<ContractAddress>::new("Fee token")
        //     .with_parser(contract_exist_parser)
        //     .with_error_message("Please enter a valid fee token (the token must exist on L1)")
        //     .prompt()?;

        // The core settlement contract on L1c.
        // Prompt the user whether to deploy the settlement contract or not.
        let settlement_contract =
            if Confirm::new("Deploy settlement contract?").with_default(true).prompt()? {
                let chain_id = cairo_short_string_to_felt(&chain_id)?;
                let initialize = deployment::deploy_settlement_contract(account, chain_id);
                let result = rt.block_on(initialize);
                result?
            }
            // If denied, prompt the user for an already deployed contract.
            else {
                // TODO: add a check to make sure the contract is indeed a valid settlement
                // contract.
                CustomType::<ContractAddress>::new("Settlement contract")
                    .with_parser(contract_exist_parser)
                    .prompt()?
            };

        Ok(PromptOutcome {
            account: account_address,
            settlement_contract,
            settlement_id: parse_cairo_short_string(&l1_chain_id)?,
            id: chain_id,
            rpc_url: settlement_url,
        })
    }
}

#[derive(Debug)]
struct PromptOutcome {
    /// the account address that is used to send the transactions for contract
    /// deployment/initialization.
    account: ContractAddress,

    // the id of the new chain to be initialized.
    id: String,

    // the chain id of the settlement layer.
    settlement_id: String,

    // the rpc url for the settlement layer.
    rpc_url: Url,

    settlement_contract: ContractAddress,
}

lazy_static! {
    static ref GENESIS: Genesis = {
        // master account
        let accounts = DevAllocationsGenerator::new(1).generate();
        let mut genesis = Genesis::default();
        genesis.extend_allocations(accounts.into_iter().map(|(k, v)| (k, v.into())));
        genesis
    };
}

const DEFAULT_APPCHAIN_FEE_TOKEN_ADDRESS: Felt =
    felt!("0x2e7442625bab778683501c0eadbc1ea17b3535da040a12ac7d281066e915eea");
