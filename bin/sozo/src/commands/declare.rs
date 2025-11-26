use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use cairo_lang_starknet_classes::casm_contract_class::CasmContractClass;
use cairo_lang_starknet_classes::contract_class::ContractClass;
use clap::Args;
use dojo_utils::{Declarer, LabeledClass, TransactionResult, TxnConfig};
use sozo_ui::SozoUi;
use starknet::core::types::contract::SierraClass;
use starknet::core::types::{Felt, FlattenedSierraClass};
use starknet_api::contract_class::compiled_class_hash::{HashVersion, HashableCompiledClass};
use tracing::trace;

use super::options::account::AccountOptions;
use super::options::starknet::StarknetOptions;
use super::options::transaction::TransactionOptions;
use crate::utils::get_account_from_env;

#[derive(Debug, Args)]
#[command(about = "Declare one or more Sierra contracts by compiling them to CASM and sending \
                   declare transactions.")]
pub struct DeclareArgs {
    #[arg(
        value_name = "SIERRA_PATH",
        num_args = 1..,
        help = "Path(s) to Sierra contract JSON artifacts (.contract_class.json)."
    )]
    pub contracts: Vec<PathBuf>,

    #[command(flatten)]
    pub transaction: TransactionOptions,

    #[command(flatten)]
    pub starknet: StarknetOptions,

    #[command(flatten)]
    pub account: AccountOptions,
}

impl DeclareArgs {
    pub async fn run(self, ui: &SozoUi) -> Result<()> {
        trace!(args = ?self);

        let DeclareArgs { contracts, transaction, starknet, account } = self;

        if contracts.is_empty() {
            return Err(anyhow!("At least one Sierra artifact path must be provided."));
        }

        let account = get_account_from_env(account, &starknet).await?;

        let use_blake2s = if let Some(rpc_url) = starknet.rpc_url {
            if rpc_url.to_string().contains("sepolia") || rpc_url.to_string().contains("testnet") {
                true
            } else {
                starknet.use_blake2s_casm_class_hash
            }
        } else {
            starknet.use_blake2s_casm_class_hash
        };

        let txn_config: TxnConfig = transaction.try_into()?;

        ui.title("Declare contracts");

        let mut prepared = Vec::new();
        for path in contracts {
            let class = prepare_class(&path, use_blake2s)
                .with_context(|| format!("Failed to prepare Sierra artifact {}", path.display()))?;

            ui.step(format!("Compiled '{}'", class.label));
            let detail_ui = ui.subsection();
            detail_ui.verbose(format!("Class hash : {:#066x}", class.class_hash));
            detail_ui.verbose(format!("CASM hash  : {:#066x}", class.casm_class_hash));
            detail_ui.verbose(format!("Artifact   : {}", path.display()));

            prepared.push(class);
        }

        let labeled = prepared
            .iter()
            .map(|class| LabeledClass {
                label: class.label.clone(),
                casm_class_hash: class.casm_class_hash,
                class: class.class.clone(),
            })
            .collect::<Vec<_>>();

        let mut declarer = Declarer::new(account, txn_config);
        declarer.extend_classes(labeled);

        let results = declarer.declare_all().await?;

        let mut declared = 0usize;
        for (class, result) in prepared.iter().zip(results.iter()) {
            match result {
                TransactionResult::Noop => {
                    ui.verbose(
                        ui.indent(1, format!("'{}' already declared on-chain.", class.label)),
                    );
                    ui.verbose(ui.indent(2, format!("Class hash: {:#066x}", class.class_hash)));
                }
                TransactionResult::Hash(hash) => {
                    declared += 1;
                    ui.result(ui.indent(1, format!("'{}' declared.", class.label)));
                    ui.verbose(ui.indent(2, format!("Class hash: {:#066x}", class.class_hash)));
                    ui.verbose(ui.indent(2, format!("Tx hash   : {hash:#066x}")));
                }
                TransactionResult::HashReceipt(hash, receipt) => {
                    declared += 1;
                    ui.result(ui.indent(1, format!("'{}' declared.", class.label)));
                    ui.verbose(ui.indent(2, format!("Class hash: {:#066x}", class.class_hash)));
                    ui.verbose(ui.indent(2, format!("Tx hash   : {hash:#066x}")));
                    ui.debug(format!("Receipt: {:?}", receipt));
                }
            }
        }

        if declared == 0 {
            ui.result("All provided classes are already declared.");
        } else {
            ui.result(format!("Declared {} class(es).", declared));
        }

        Ok(())
    }
}

#[derive(Debug)]
struct PreparedClass {
    label: String,
    class_hash: Felt,
    casm_class_hash: Felt,
    class: FlattenedSierraClass,
}

fn prepare_class(path: &Path, use_blake2s: bool) -> Result<PreparedClass> {
    let data = fs::read(path)?;

    let sierra: SierraClass = serde_json::from_slice(&data)?;
    let class_hash = sierra.class_hash()?;
    let flattened = sierra.clone().flatten()?;

    let casm_hash = casm_class_hash_from_bytes(&data, use_blake2s)?;

    let label = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .ok_or_else(|| anyhow!("Unable to infer contract name from {}", path.display()))?
        .to_string();

    Ok(PreparedClass { label, class_hash, casm_class_hash: casm_hash, class: flattened })
}

fn casm_class_hash_from_bytes(data: &[u8], use_blake2s: bool) -> Result<Felt> {
    let sierra_class: ContractClass = serde_json::from_slice(data)?;
    let casm_class = CasmContractClass::from_contract_class(sierra_class, false, usize::MAX)?;

    let hash_version = if use_blake2s { HashVersion::V2 } else { HashVersion::V1 };
    let hash = casm_class.hash(&hash_version);

    Ok(Felt::from_bytes_be(&hash.0.to_bytes_be()))
}
